// vim: tw=80
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    num::NonZeroU64,
    os::fd::AsRawFd,
    path::PathBuf,
    process
};

use libc::c_void;
use log::{debug, error, info};
use nix::sys::mman::{ProtFlags, MapFlags, MsFlags, mmap, munmap, msync};
use rand::{         
    Rng,            
    RngCore,
    SeedableRng,
    distributions::{Distribution, Standard},
    thread_rng
};  

use clap::Parser;

mod prng;
use prng::OsPRng;

const MAXFILELEN: u64 = 256 * 1024;
const MAXOPLEN: usize = 64 * 1024;

#[derive(Debug, Parser)]
//#[command(author, version, about, long_about = None)]
struct Cli {
    /// File name to operate on
    fname: PathBuf,

    /// Beginning operation number
    #[arg(short = 'b', default_value_t = NonZeroU64::new(1u64).unwrap())]
    opnum: NonZeroU64,

    /// 1/P chance of file close+open at each op (default infinity)
    #[arg(short = 'c', value_name = "P")]
    closeprob: Option<u64>,

    /// Disable msync after mapwrite
    #[arg(short = 'U')]
    nomsyncafterwrite: bool,

    /// Disable mmap reads
    #[arg(short = 'R')]
    nomapread: bool,

    /// Disable mmap writes
    #[arg(short = 'W')]
    nomapwrite: bool,

    /// Total number of operations to do (default infinity)
    #[arg(short = 'N')]
    numops: Option<u64>,
    /// Seed for RNG
    #[arg(short = 'S')]
    seed: Option<u32>
    // TODO
    // -i
    // -l
    // -m
    // -n
    // -o
    // -p
    // -q
    // -r
    // -s
    // -t
    // -w
    // -D
    // -L
    // -O
    // -P
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Op {
    Read,
    Write,
    MapRead,
    Truncate,
    MapWrite
}

impl Distribution<Op> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Op {
        // Manually handle the modulo division, rather than using
        // RngCore::gen_range, for compatibility with the C-based FSX
        let x = rng.next_u64() % 5;
        match x {
            0 => Op::Read,
            1 => Op::Write,
            2 => Op::MapRead,
            3 => Op::Truncate,
            4 => Op::MapWrite,
            _ => unreachable!()
        }
    }
}
struct Exerciser {
    /// Current file size
    file_size: u64,
    fname: PathBuf,
    // What the file ought to contain
    good_buf: Vec<u8>,
    nomapread: bool,
    nomapwrite: bool,
    nomsyncafterwrite: bool,
    numops: Option<u64>,
    // 0-indexed operation number to begin real transfers.
    simulatedopcount: u64,
    // File's original data
    original_buf: Vec<u8>,
    // Use OsPRng for full backwards-compatibility with the C fsx
    rng: OsPRng,
    // Number of steps completed so far
    steps: u64,
    file: File
}

impl Exerciser {
    fn check_buffers(&self, buf: &[u8], offset: u64) {
        let size = buf.len();
        if &self.good_buf[offset as usize..offset as usize + size] != &buf[..] {
            error!("miscompare: offset= {:#x}, size = {:#x}", offset, size);
            // TODO: detailed comparison
            process::exit(1);
        }
    }

    fn check_eofpage(offset: u64, file_size: u64, p: *const c_void, size: usize)
    {
        let page_size = Self::getpagesize() as usize;
        let page_mask =  page_size as isize - 1;
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
                as isize & !page_mask)
                as *const u8;
            std::slice::from_raw_parts(last_page_p, page_size)
        };
        for (i, b) in last_page[file_size as usize & page_mask as usize..].iter().enumerate() {
            if *b != 0 {
                error!("Mapped non-zero data past EoF ({:#x}) page offset {:#x} is {:#x}",
                    file_size - 1,
                    (file_size & page_mask as u64) + i as u64,
                    *b
                );
                process::exit(1);
            }
        }
    }

    fn check_size(&self) {
        let size = self.file.metadata()
            .unwrap()
            .len();
        assert_eq!(size, self.file_size);
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
                offset as i64 - pg_offset as i64
            ).unwrap();
            (p as *mut u8).offset(pg_offset as isize)
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
                offset as i64 - pg_offset as i64
            ).unwrap();
            ((p as *mut u8).offset(pg_offset as isize))
                .copy_from(buf.as_ptr(), size);
            if ! self.nomsyncafterwrite {
                msync(p, map_size, MsFlags::MS_SYNC).unwrap();
            }
            Self::check_eofpage(offset, self.file_size, p, size);
            munmap(p, map_size).unwrap();
        }
    }

    fn dowrite(&mut self, _cur_file_size: u64, size: usize, offset: u64) {
        let buf = &self.good_buf[offset as usize..offset as usize + size];
        self.file.seek(SeekFrom::Start(offset)).unwrap();
        let written = self.file.write(&buf).unwrap();
        if written != size {
            error!("short write: {:#x} bytes instead of {:#x}", written, size);
            process::exit(1);
        }
    }

    /// Wrapper around read-like operations
    fn read_like<F>(&mut self, op: &str, offset: u64, size: usize, f: F)
        where F: Fn(&mut Exerciser, &mut [u8], u64, usize)
    {
        if size == 0 {
            debug!("{} skipping zero size read", self.steps);
            return;
        }
        if size as u64 + offset > self.file_size {
            debug!("{} skipping seek/read past EoF", self.steps);
            return;
        }
        if self.steps <= self.simulatedopcount {
            return;
        }
        info!("{} {} {:#x} thru {:#x} ({:#x} bytes)",
            self.steps,
            op,
            offset,
            offset + size as u64 - 1,
            size);
        let mut temp_buf = vec![0u8; size];
        f(self, &mut temp_buf[..], offset, size);
        self.check_buffers(&temp_buf, offset)
    }

    /// Wrapper around write-like operations.
    fn write_like<F>(&mut self, op: &str, offset: u64, size: usize, f: F)
        where F: Fn(&mut Exerciser, u64, usize, u64)
    {
        if size == 0 {
            debug!("{} skipping zero size write", self.steps);
            return;
        }

        self.gendata(offset, size);

        let cur_file_size = self.file_size;
        if self.file_size < offset + size as u64 {
            if self.file_size < offset {
                safemem::write_bytes(&mut self.good_buf[self.file_size as usize ..offset as usize], 0)
            }
            self.file_size = offset + size as u64;
        }

        if self.steps <= self.simulatedopcount {
            return;
        }

        info!("{} {} {:#x} thru {:#x} ({:#x} bytes)",
            self.steps,
            op,
            offset,
            offset + size as u64 - 1,
            size);

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
            if size == 0 {
                break;
            }
            self.good_buf[uoff] = (self.steps % 256) as u8;
            if uoff % 2 > 0 {
                self.good_buf[uoff] = self.good_buf[uoff].wrapping_add(self.original_buf[uoff]);
            }
            uoff += 1;
        }
    }

    fn getpagesize() -> i32 {
        // This function is inherently safe
        unsafe { libc::getpagesize() }
    }

    fn mapread(&mut self, offset: u64, size: usize) {
        self.read_like(&"mapread", offset, size, Self::domapread)
    }

    fn mapwrite(&mut self, offset: u64, size: usize) {
        self.write_like(&"mapwrite", offset, size, Self::domapwrite)
    }

    fn read(&mut self, offset: u64, size: usize) {
        self.read_like(&"read", offset, size, Self::doread)
    }

    fn step(&mut self) {
        let op: Op = self.rng.gen();

        if self.simulatedopcount > 0 && self.steps == self.simulatedopcount {
            self.writefileimage();
        }
        self.steps += 1;

        let mut size = MAXOPLEN;
        if op == Op::Write || op == Op::MapWrite {
            let mut offset: u64 = self.rng.gen();
            offset %= MAXFILELEN;
            if offset + size as u64 > MAXFILELEN {
                size = usize::try_from(MAXFILELEN - offset).unwrap();
            }
            if !self.nomapwrite && op == Op::MapWrite {
                self.mapwrite(offset, size);
            } else {
                self.write(offset, size);
            }
        } else if op == Op::Truncate {
            let fsize = self.rng.gen::<u64>() % MAXFILELEN;
            self.truncate(fsize)
        } else {
            let mut offset: u64 = self.rng.gen();
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
    }

    fn truncate(&mut self, size: u64) {
        if size > self.file_size {
            safemem::write_bytes(&mut self.good_buf[self.file_size as usize ..size as usize], 0)
        }
        let cur_file_size = self.file_size;
        self.file_size = size;

        if self.steps <= self.simulatedopcount {
            return;
        }

        info!("{} truncate from {:#x} to {:#x}", self.steps, cur_file_size, size);
        self.file.set_len(size).unwrap();
    }

    fn write(&mut self, offset: u64, size: usize) {
        self.write_like(&"write", offset, size, Self::dowrite)
    }

    fn writefileimage(&mut self) {
        self.file.seek(SeekFrom::Start(0)).unwrap();
        let written = self.file.write(&self.good_buf[..self.file_size as usize]).unwrap();
        if written as u64 != self.file_size {
            error!("short write: {:#x} bytes instead of {:#x}", written,
                   self.file_size);
            process::exit(1);
        }
        self.file.set_len(self.file_size).unwrap();
    }
}

impl From<Cli> for Exerciser {
    fn from(cli: Cli) -> Self {
        let seed = cli.seed.unwrap_or_else(|| {
            let mut seeder = thread_rng();
            seeder.gen()
        });
        info!("Using seed {}", seed);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&cli.fname)
            .expect("Cannot create file");
        let mut original_buf = vec![0u8; MAXFILELEN as usize];
        let good_buf = vec![0u8; MAXFILELEN as usize];
        let mut rng = OsPRng::from_seed(seed.to_ne_bytes());
        rng.fill_bytes(&mut original_buf[..]);
        Exerciser{
            file,
            file_size: 0,
            fname: cli.fname,
            good_buf,
            nomapread: cli.nomapread,
            nomapwrite: cli.nomapwrite,
            nomsyncafterwrite: cli.nomsyncafterwrite,
            numops: cli.numops,
            simulatedopcount: <NonZeroU64 as Into<u64>>::into(cli.opnum) - 1,
            original_buf,
            rng,
            steps: 0
        }
    }
}

fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .init();
    let cli = Cli::parse();
    let mut exerciser = Exerciser::from(cli);
    exerciser.exercise()
}
