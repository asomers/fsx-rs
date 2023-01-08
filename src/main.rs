// vim: tw=80
use std::{
    ffi::OsStr,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    mem,
    num::NonZeroU64,
    os::fd::{AsRawFd, IntoRawFd},
    path::PathBuf,
    process,
};

use clap::{
    builder::TypedValueParser,
    error::ErrorKind,
    Arg,
    Command,
    Error,
    Parser,
};
use libc::c_void;
use log::{debug, error, info, log, Level};
use nix::{
    sys::mman::{mmap, msync, munmap, MapFlags, MsFlags, ProtFlags},
    unistd::{sysconf, SysconfVar},
};
use rand::{
    distributions::{Distribution, Standard},
    thread_rng,
    Rng,
    RngCore,
    SeedableRng,
};

mod prng;
use prng::OsPRng;

/// Calculate the maximum field width needed to print numbers up to this size
fn field_width(max: usize, hex: bool) -> usize {
    if hex {
        2 + (8 * mem::size_of_val(&max) - max.leading_zeros() as usize + 3) / 4
    } else {
        1 + (max as f64).log(10.0) as usize
    }
}

#[derive(Clone)]
struct MonitorParser {}
impl TypedValueParser for MonitorParser {
    type Value = (u64, u64);

    fn parse_ref(
        &self,
        cmd: &Command,
        _arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, Error> {
        let vs = value.to_str().ok_or_else(|| {
            clap::Error::new(ErrorKind::InvalidUtf8).with_cmd(cmd)
        })?;
        let fields = vs.split(':').collect::<Vec<_>>();
        if fields.len() != 2 {
            let e = clap::Error::raw(
                ErrorKind::InvalidValue,
                "-m argument must contain exactly one ':'",
            )
            .with_cmd(cmd);
            return Err(e);
        }
        let startop = fields[0].parse::<u64>().map_err(|_| {
            clap::Error::raw(
                ErrorKind::InvalidValue,
                "-m arguments must be numeric",
            )
        })?;
        let endop = fields[1].parse::<u64>().map_err(|_| {
            clap::Error::raw(
                ErrorKind::InvalidValue,
                "-m arguments must be numeric",
            )
        })?;
        Ok((startop, endop))
    }
}

#[derive(Debug, Parser)]
//#[command(author, version, about, long_about = None)]
struct Cli {
    // TODO
    // -L
    // -P
    /// File name to operate on
    fname: PathBuf,

    /// Maximum file size
    // NB: could be u64, but the C-based FSX only works with 32-bit file sizes
    #[arg(short = 'l', default_value_t = 256 * 1024)]
    flen: u32,

    /// Inject an error on step N
    #[arg(long = "inject", hide = true, value_name = "N")]
    inject: Option<u64>,

    /// Beginning operation number
    #[arg(short = 'b', default_value_t = NonZeroU64::new(1u64).unwrap())]
    opnum: NonZeroU64,

    /// 1/P chance of file close+open at each op (default infinity)
    #[arg(short = 'c', value_name = "P")]
    closeprob: Option<u32>,

    /// 1/P chance of msync(MS_INVALIDATE) (default infinity)
    #[arg(short = 'i', value_name = "P")]
    invalprob: Option<u32>,

    /// Monitor specified byte range
    #[arg(short = 'm', value_name = "from:to", value_parser = MonitorParser{})]
    monitor: Option<(u64, u64)>,

    /// Disable msync after mapwrite
    #[arg(short = 'U')]
    nomsyncafterwrite: bool,

    /// Disable mmap reads
    #[arg(short = 'R')]
    nomapread: bool,

    /// Disable mmap writes
    #[arg(short = 'W')]
    nomapwrite: bool,

    /// Use oplen (see -o flag) for every op (default random)
    #[arg(short = 'O')]
    norandomoplen: bool,

    /// Total number of operations to do (default infinity)
    #[arg(short = 'N')]
    numops: Option<u64>,

    /// Maximum size for operations
    #[arg(short = 'o', default_value_t = 65536)]
    oplen: usize,

    /// Read boundary. 4k to make reads page aligned
    #[arg(short = 'r', default_value_t = 1)]
    readbdy: u64,

    /// Seed for RNG
    #[arg(short = 'S')]
    seed: Option<u32>,

    /// Trunc boundary. 4k to make truncs page aligned
    #[arg(short = 't', default_value_t = 1)]
    truncbdy: u64,

    /// Disable verifications of file size
    #[arg(short = 'n')]
    nosizechecks: bool,

    /// Write boundary. 4k to make writes page aligned
    #[arg(short = 'w', default_value_t = 1)]
    writebdy: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Op {
    Read,
    Write,
    MapRead,
    Truncate,
    MapWrite,
}

impl From<u32> for Op {
    fn from(rv: u32) -> Self {
        let x = rv % 5;
        match x {
            0 => Op::Read,
            1 => Op::Write,
            2 => Op::MapRead,
            3 => Op::Truncate,
            4 => Op::MapWrite,
            _ => unreachable!(),
        }
    }
}

impl Distribution<Op> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Op {
        // Manually handle the modulo division, rather than using
        // RngCore::gen_range, for compatibility with the C-based FSX
        Op::from(rng.next_u32())
    }
}
struct Exerciser {
    /// 1 in P chance of close+open at each op
    closeprob:         Option<u32>,
    /// Current file size
    file_size:         u64,
    flen:              u64,
    fname:             PathBuf,
    /// Width for printing fields containing file offsets
    fwidth:            usize,
    /// Inject an error on this step
    inject:            Option<u64>,
    /// 1 in P chance of MS_INVALIDATE at each op
    invalprob:         Option<u32>,
    // What the file ought to contain
    good_buf:          Vec<u8>,
    maxoplen:          usize,
    /// Monitor these byte ranges in extra detail.
    monitor:           Option<(u64, u64)>,
    nomapread:         bool,
    nomapwrite:        bool,
    nomsyncafterwrite: bool,
    norandomoplen:     bool,
    nosizechecks:      bool,
    numops:            Option<u64>,
    readbdy:           u64,
    // 0-indexed operation number to begin real transfers.
    simulatedopcount:  u64,
    /// Width for printing fields containing operation sizes
    swidth:            usize,
    /// Width for printing the step number field
    stepwidth:         usize,
    // File's original data
    original_buf:      Vec<u8>,
    // Use OsPRng for full backwards-compatibility with the C fsx
    rng:               OsPRng,
    // Number of steps completed so far
    steps:             u64,
    file:              File,
    truncbdy:          u64,
    writebdy:          u64,
}

impl Exerciser {
    fn check_buffers(&self, buf: &[u8], mut offset: u64) {
        let mut size = buf.len();
        if self.good_buf[offset as usize..offset as usize + size] != buf[..] {
            error!("miscompare: offset= {:#x}, size = {:#x}", offset, size);
            let mut i = 0;
            let mut n = 0;
            let mut good = 0;
            let mut bad = 0;
            let mut badoffset = 0;
            let mut op = 0;
            error!(
                "{:fwidth$} GOOD  BAD  {:swidth$}",
                "OFFSET",
                "RANGE",
                fwidth = self.fwidth,
                swidth = self.swidth
            );
            while size > 0 {
                let c = self.good_buf[offset as usize];
                let t = buf[i];
                if c != t {
                    if n == 0 {
                        good = c;
                        bad = t;
                        badoffset = offset;
                        op = buf[if offset & 1 != 0 { i + 1 } else { i }];
                    }
                    n += 1;
                }
                offset += 1;
                i += 1;
                size -= 1;
            }
            assert!(n > 0);
            // XXX The reported range may be a little too small, because
            // some bytes in the damaged range may coincidentally match.  But
            // this is the way that the C-based FSX reported it.
            error!(
                "{:#fwidth$x} {:#04x} {:#04x} {:#swidth$x}",
                badoffset,
                good,
                bad,
                n,
                fwidth = self.fwidth,
                swidth = self.swidth
            );
            if op > 0 {
                error!("Step# (mod 256) for a misdirected write may be {}", op);
            } else {
                error!(
                    "Step# for the bad data is unknown; check HOLE and EXTEND \
                     ops"
                );
            }
            process::exit(1);
        }
    }

    fn check_eofpage(
        offset: u64,
        file_size: u64,
        p: *const c_void,
        size: usize,
    ) {
        let page_size = Self::getpagesize() as usize;
        let page_mask = page_size as isize - 1;
        if offset + size as u64 <= file_size & !(page_mask as u64) {
            return;
        }

        // We landed in the last page of the file.  Test to make sure the VM
        // system provided 0's beyond the true end of the file mapping (as
        // required by mmap def in 1996 posix 1003.1).
        //
        // Safety: mmap always maps to the end of a page, and we drop the slice
        // before munmap().
        let last_page = unsafe {
            let last_page_p = ((p as *mut u8)
                .offset((offset as isize & page_mask) + size as isize)
                as isize
                & !page_mask) as *const u8;
            std::slice::from_raw_parts(last_page_p, page_size)
        };
        for (i, b) in last_page[file_size as usize & page_mask as usize..]
            .iter()
            .enumerate()
        {
            if *b != 0 {
                error!(
                    "Mapped non-zero data past EoF ({:#x}) page offset {:#x} \
                     is {:#x}",
                    file_size - 1,
                    (file_size & page_mask as u64) + i as u64,
                    *b
                );
                process::exit(1);
            }
        }
    }

    fn check_size(&mut self) {
        if !self.nosizechecks {
            let size = self.file.metadata().unwrap().len();
            let size_by_seek = self.file.seek(SeekFrom::End(0)).unwrap();
            if size != self.file_size || size_by_seek != self.file_size {
                error!(
                    "Size error: expected {:#x} but found {:#x} by stat and \
                     {:#x} by seek",
                    self.file_size, size, size_by_seek
                );
                process::exit(1);
            }
        }
    }

    /// Close and reopen the file
    fn closeopen(&mut self) {
        if self.skip() {
            return;
        }
        info!("{:width$} close/open", self.steps, width = self.stepwidth);

        // We must remove and drop the old File before opening it, and that
        // requires swapping its contents.
        // Safe because we never access the uninitialized File object.
        unsafe {
            let placeholder: File = mem::MaybeUninit::zeroed().assume_init();
            drop(mem::replace(&mut self.file, placeholder));
            let newfile = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.fname)
                .expect("Cannot open file");
            let placeholder = mem::replace(&mut self.file, newfile);
            placeholder.into_raw_fd();
        }
    }

    fn doread(&mut self, buf: &mut [u8], offset: u64, size: usize) {
        self.file.seek(SeekFrom::Start(offset)).unwrap();
        let read = self.file.read(buf).unwrap();
        if read < size {
            error!("short read: {:#x} bytes instead of {:#x}", read, size);
            process::exit(1);
        }
    }

    fn domapread(&mut self, buf: &mut [u8], offset: u64, size: usize) {
        let page_mask = Self::getpagesize() as usize - 1;
        let pg_offset = offset as usize & page_mask;
        let map_size = pg_offset + size;
        unsafe {
            let p = mmap(
                None,
                map_size.try_into().unwrap(),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_FILE | MapFlags::MAP_SHARED,
                self.file.as_raw_fd(),
                offset as i64 - pg_offset as i64,
            )
            .unwrap();
            (p as *mut u8)
                .add(pg_offset)
                .copy_to(buf.as_mut_ptr(), size);
            Self::check_eofpage(offset, self.file_size, p, size);
        }
    }

    fn domapwrite(&mut self, cur_file_size: u64, size: usize, offset: u64) {
        if self.file_size > cur_file_size {
            self.file.set_len(self.file_size).unwrap();
        }
        let buf = &self.good_buf[offset as usize..offset as usize + size];
        let page_mask = Self::getpagesize() as usize - 1;
        let pg_offset = offset as usize & page_mask;
        let map_size = pg_offset + size;
        // Safety: good luck proving it's safe.
        unsafe {
            let p = mmap(
                None,
                map_size.try_into().unwrap(),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_FILE | MapFlags::MAP_SHARED,
                self.file.as_raw_fd(),
                offset as i64 - pg_offset as i64,
            )
            .unwrap();
            ((p as *mut u8).add(pg_offset)).copy_from(buf.as_ptr(), size);
            if !self.nomsyncafterwrite {
                msync(p, map_size, MsFlags::MS_SYNC).unwrap();
            }
            Self::check_eofpage(offset, self.file_size, p, size);
            munmap(p, map_size).unwrap();
        }
    }

    fn dowrite(&mut self, _cur_file_size: u64, size: usize, offset: u64) {
        let buf = &self.good_buf[offset as usize..offset as usize + size];
        self.file.seek(SeekFrom::Start(offset)).unwrap();
        let written = self.file.write(buf).unwrap();
        if written != size {
            error!("short write: {:#x} bytes instead of {:#x}", written, size);
            process::exit(1);
        }
    }

    /// Wrapper around read-like operations
    fn read_like<F>(&mut self, op: &str, mut offset: u64, size: usize, f: F)
    where
        F: Fn(&mut Exerciser, &mut [u8], u64, usize),
    {
        offset -= offset % self.readbdy;

        if size == 0 {
            debug!(
                "{:width$} skipping zero size read",
                self.steps,
                width = self.stepwidth
            );
            return;
        }
        if size as u64 + offset > self.file_size {
            debug!(
                "{:width$} skipping seek/read past EoF",
                self.steps,
                width = self.stepwidth
            );
            return;
        }
        if self.skip() {
            return;
        }
        let loglevel = self.loglevel(offset, size);
        log!(
            loglevel,
            "{:stepwidth$} {:8} {:#fwidth$x} .. {:#fwidth$x} ({:#swidth$x} \
             bytes)",
            self.steps,
            op,
            offset,
            offset + size as u64 - 1,
            size,
            stepwidth = self.stepwidth,
            fwidth = self.fwidth,
            swidth = self.swidth
        );
        let mut temp_buf = vec![0u8; size];
        f(self, &mut temp_buf[..], offset, size);
        self.check_buffers(&temp_buf, offset)
    }

    /// Should this step be skipped as not part of the test plan?
    fn skip(&self) -> bool {
        self.steps <= self.simulatedopcount || Some(self.steps) == self.inject
    }

    /// Wrapper around write-like operations.
    fn write_like<F>(&mut self, op: &str, mut offset: u64, size: usize, f: F)
    where
        F: Fn(&mut Exerciser, u64, usize, u64),
    {
        offset -= offset % self.writebdy;

        if size == 0 {
            debug!(
                "{:width$} skipping zero size write",
                self.steps,
                width = self.stepwidth
            );
            return;
        }

        self.gendata(offset, size);

        let cur_file_size = self.file_size;
        if self.file_size < offset + size as u64 {
            if self.file_size < offset {
                safemem::write_bytes(
                    &mut self.good_buf
                        [self.file_size as usize..offset as usize],
                    0,
                )
            }
            self.file_size = offset + size as u64;
        }

        if self.skip() {
            return;
        }

        let loglevel = self.loglevel(offset, size);
        log!(
            loglevel,
            "{:stepwidth$} {:8} {:#fwidth$x} .. {:#fwidth$x} ({:#swidth$x} \
             bytes)",
            self.steps,
            op,
            offset,
            offset + size as u64 - 1,
            size,
            stepwidth = self.stepwidth,
            fwidth = self.fwidth,
            swidth = self.swidth
        );

        f(self, cur_file_size, size, offset)
    }

    fn exercise(&mut self) {
        loop {
            if let Some(n) = self.numops {
                if n <= self.steps {
                    break;
                }
            }
            self.step();
        }

        println!("All operations completed A-OK!");
    }

    fn gendata(&mut self, offset: u64, mut size: usize) {
        let mut uoff = usize::try_from(offset).unwrap();
        loop {
            size -= 1;
            self.good_buf[uoff] = (self.steps % 256) as u8;
            if uoff % 2 > 0 {
                self.good_buf[uoff] =
                    self.good_buf[uoff].wrapping_add(self.original_buf[uoff]);
            }
            uoff += 1;
            if size == 0 {
                break;
            }
        }
    }

    fn getpagesize() -> i32 {
        // This function is inherently safe
        sysconf(SysconfVar::PAGE_SIZE).unwrap().unwrap() as i32
    }

    fn invalidate(&self) {
        if self.skip() {
            return;
        }
        info!(
            "{:width$} msync(MS_INVALIDATE)",
            self.steps,
            width = self.stepwidth
        );
        let len = self.file_size as usize;
        unsafe {
            let p = mmap(
                None,
                len.try_into().unwrap(),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_FILE | MapFlags::MAP_SHARED,
                self.file.as_raw_fd(),
                0,
            )
            .unwrap();
            msync(p, 0, MsFlags::MS_INVALIDATE).unwrap();
            munmap(p, len).unwrap();
        }
    }

    /// Log level to use for I/O operations.
    fn loglevel(&self, offset: u64, size: usize) -> Level {
        let mut loglevel = Level::Info;
        if let Some((start, end)) = self.monitor {
            if start < offset + size as u64 && offset <= end {
                loglevel = Level::Warn;
            }
        }
        loglevel
    }

    fn mapread(&mut self, offset: u64, size: usize) {
        self.read_like("mapread", offset, size, Self::domapread)
    }

    fn mapwrite(&mut self, offset: u64, size: usize) {
        self.write_like("mapwrite", offset, size, Self::domapwrite)
    }

    fn read(&mut self, offset: u64, size: usize) {
        self.read_like("read", offset, size, Self::doread)
    }

    fn step(&mut self) {
        // It would be more natural to generate op and closeopen independently.
        // But do it this way for backwards compatibility with C.
        let rv: u32 = self.rng.gen();
        let op = if self.nomapwrite {
            // Sigh.  Yes, this is how C does it.
            Op::from(rv % 4)
        } else {
            Op::from(rv)
        };

        if self.simulatedopcount > 0 && self.steps == self.simulatedopcount {
            self.writefileimage();
        }
        self.steps += 1;

        let closeopen = if let Some(x) = self.closeprob {
            (rv >> 3) < (1 << 28) / x
        } else {
            false
        };
        let invl = if let Some(x) = self.invalprob {
            (rv >> 3) < (1 << 28) / x
        } else {
            false
        };

        if op == Op::Write || op == Op::MapWrite {
            let mut size = if self.norandomoplen {
                self.maxoplen
            } else {
                self.rng.gen::<u32>() as usize % (self.maxoplen + 1)
            };
            let mut offset: u64 = self.rng.gen::<u32>() as u64;
            offset %= self.flen;
            if offset + size as u64 > self.flen {
                size = usize::try_from(self.flen - offset).unwrap();
            }
            if !self.nomapwrite && op == Op::MapWrite {
                self.mapwrite(offset, size);
            } else {
                self.write(offset, size);
            }
        } else if op == Op::Truncate {
            let fsize = u64::from(self.rng.gen::<u32>()) % self.flen;
            self.truncate(fsize)
        } else {
            let mut size = if self.norandomoplen {
                self.maxoplen
            } else {
                self.rng.gen::<u32>() as usize % (self.maxoplen + 1)
            };
            let mut offset: u64 = self.rng.gen::<u32>() as u64;
            offset = if self.file_size > 0 {
                offset % self.file_size
            } else {
                0
            };
            if offset + size as u64 > self.file_size {
                size = usize::try_from(self.file_size - offset).unwrap();
            }
            if !self.nomapread && op == Op::MapRead {
                self.mapread(offset, size);
            } else {
                self.read(offset, size);
            }
        }
        if self.steps > self.simulatedopcount {
            self.check_size();
        }
        if invl {
            self.invalidate()
        }
        if closeopen {
            self.closeopen()
        }
    }

    fn truncate(&mut self, mut size: u64) {
        size -= size % self.truncbdy;

        if size > self.file_size {
            safemem::write_bytes(
                &mut self.good_buf[self.file_size as usize..size as usize],
                0,
            )
        }
        let cur_file_size = self.file_size;
        self.file_size = size;

        if self.skip() {
            return;
        }

        // XXX Should not log at WARN if size < self.monitor.0 and
        // self.file_size < self.monitor.0.  But the C-based implementation
        // does.
        let mut loglevel = Level::Info;
        if let Some((_, end)) = self.monitor {
            if size <= end {
                loglevel = Level::Warn;
            }
        }
        log!(
            loglevel,
            "{:stepwidth$} truncate {:#fwidth$x} => {:#fwidth$x}",
            self.steps,
            cur_file_size,
            size,
            stepwidth = self.stepwidth,
            fwidth = self.fwidth
        );
        self.file.set_len(size).unwrap();
    }

    fn write(&mut self, offset: u64, size: usize) {
        self.write_like("write", offset, size, Self::dowrite)
    }

    fn writefileimage(&mut self) {
        self.file.seek(SeekFrom::Start(0)).unwrap();
        let written = self
            .file
            .write(&self.good_buf[..self.file_size as usize])
            .unwrap();
        if written as u64 != self.file_size {
            error!(
                "short write: {:#x} bytes instead of {:#x}",
                written, self.file_size
            );
            process::exit(1);
        }
        self.file.set_len(self.file_size).unwrap();
    }
}

impl From<Cli> for Exerciser {
    fn from(cli: Cli) -> Self {
        let seed = cli.seed.unwrap_or_else(|| {
            let mut seeder = thread_rng();
            // The legacy FSX only uses 31-bit seeds.
            seeder.gen::<u32>() & 0x7FFFFFFF
        });
        info!("Using seed {}", seed);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&cli.fname)
            .expect("Cannot create file");
        let mut original_buf = vec![0u8; cli.flen as usize];
        let good_buf = vec![0u8; cli.flen as usize];
        let mut rng = OsPRng::from_seed(seed.to_ne_bytes());
        rng.fill_bytes(&mut original_buf[..]);
        let fwidth = field_width(cli.flen as usize, true);
        let swidth = field_width(cli.oplen, true);
        let stepwidth = field_width(
            cli.numops.map(|x| x as usize).unwrap_or(999999),
            false,
        );
        Exerciser {
            closeprob: cli.closeprob,
            file,
            file_size: 0,
            flen: cli.flen.into(),
            fwidth,
            fname: cli.fname,
            good_buf,
            inject: cli.inject,
            invalprob: cli.invalprob,
            maxoplen: cli.oplen,
            monitor: cli.monitor,
            nomapread: cli.nomapread,
            nomapwrite: cli.nomapwrite,
            nomsyncafterwrite: cli.nomsyncafterwrite,
            norandomoplen: cli.norandomoplen,
            nosizechecks: cli.nosizechecks,
            numops: cli.numops,
            readbdy: cli.readbdy,
            simulatedopcount: <NonZeroU64 as Into<u64>>::into(cli.opnum) - 1,
            swidth,
            stepwidth,
            original_buf,
            rng,
            steps: 0,
            truncbdy: cli.truncbdy,
            writebdy: cli.writebdy,
        }
    }
}

fn main() {
    env_logger::builder().format_timestamp(None).init();
    let cli = Cli::parse();
    let mut exerciser = Exerciser::from(cli);
    exerciser.exercise()
}
