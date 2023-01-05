// vim: tw=80
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
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
use rand_xorshift::XorShiftRng;

use clap::Parser;

const MAXFILELEN: u64 = 256 * 1024;
const MAXOPLEN: usize = 64 * 1024;

#[derive(Debug, Parser)]
//#[command(author, version, about, long_about = None)]
struct Cli {
    /// File name to operate on
    fname: PathBuf,

    /// Beginning operation number
    #[arg(short = 'b', default_value_t = 1u64)]
    opnum: u64,

    /// 1/P chance of file close+open at each op (default infinity)
    #[arg(short = 'c', value_name = "P")]
    closeprob: Option<u64>,

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
    seed: Option<u64>
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
    // -U
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
        match rng.gen_range(0..=4) {
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
    numops: Option<u64>,
    opnum: u64,
    // File's original data
    original_buf: Vec<u8>,
    // Use XorShiftRng because it's deterministic and seedable.
    // XXX It might be nicer to use random(3) for full compatibility with the C
    // implementation.
    rng: XorShiftRng,
    seed: u64,
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

    fn doread<F>(&mut self, op: &str, offset: u64, size: usize, f: F)
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
        info!("{} {} {:#x} thru {:#x} ({:#x} bytes)",
            self.steps,
            op,
            offset,
            offset + size as u64,
            size);
        let mut temp_buf = vec![0u8; size];
        f(self, &mut temp_buf[..], offset, size);
        self.check_buffers(&temp_buf, offset)
    }

    fn dowrite<F>(&mut self, op: &str, offset: u64, size: usize, f: F)
        where F: Fn(&File, u64, u64, &[u8], u64)
    {
        if size == 0 {
            debug!("{} skipping zero size write", self.steps);
            return;
        }

        info!("{} {} {:#x} thru {:#x} ({:#x} bytes)",
            self.steps,
            op,
            offset,
            offset + size as u64,
            size);

        self.gendata(offset, size);

        let cur_file_size = self.file_size;
        if self.file_size < offset + size as u64 {
            if self.file_size < offset {
                safemem::write_bytes(&mut self.good_buf[self.file_size as usize ..offset as usize], 0)
            }
            self.file_size = offset + size as u64;
        }
        f(&self.file, cur_file_size, self.file_size, &self.good_buf[offset as usize..offset as usize + size], offset)
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

    fn mapwrite(&mut self, offset: u64, size: usize) {
        self.dowrite(&"mapwrite", offset, size,
             |file, cur_file_size, file_size, buf, offset| {
                if file_size > cur_file_size {
                    file.set_len(file_size).unwrap();
                }
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
                        file.as_raw_fd(),
                        offset as i64 - pg_offset as i64
                    ).unwrap();
                    ((p as *mut u8).offset(pg_offset as isize))
                        .copy_from(buf.as_ptr(), buf.len());
                    msync(p, map_size, MsFlags::MS_SYNC).unwrap();
                    Self::check_eofpage(offset, file_size, p, size);
                    munmap(p, map_size).unwrap();
                }
            }
        )
    }

    fn doread1(&mut self, buf: &mut [u8], offset: u64, size: usize) {
        self.file.seek(SeekFrom::Start(offset)).unwrap();
        let read = self.file.read(buf).unwrap();
        if read < size {
            error!("short read: {:#x} bytes instead of {:#x}", read, size);
            process::exit(1);
        }
    }

    fn read(&mut self, offset: u64, size: usize) {
        self.doread(&"read", offset, size, Self::doread1)
    }

    fn step(&mut self) {
        let op: Op = self.rng.gen();
        self.steps += 1;

        let mut size = MAXOPLEN;
        let mut offset: u64 = self.rng.gen();
        if op == Op::Write || op == Op::MapWrite {
            offset %= MAXFILELEN;
            if offset + size as u64 > MAXFILELEN {
                size = usize::try_from(MAXFILELEN - offset).unwrap();
            }
            if !self.nomapwrite && op == Op::MapWrite {
                self.mapwrite(offset, size);
            } else {
                self.write(offset, size);
            }
        } else {
            offset = if self.file_size > 0 {
                offset % self.file_size
            } else {
                0
            };
            if offset + size as u64 > MAXFILELEN {
                size = usize::try_from(self.file_size - offset).unwrap();
            }
            if !self.nomapread && op == Op::MapRead {
                todo!()
            } else {
                self.read(offset, size);
            }
        }
    }

    fn write(&mut self, offset: u64, size: usize) {
        self.dowrite(&"write", offset, size, |mut file, _, _, buf, offset| {
            let size = buf.len();
            file.seek(SeekFrom::Start(offset)).unwrap();
            let written = file.write(&buf).unwrap();
            if written != size {
                error!("short write: {:#x} bytes instead of {:#x}", written, size);
                process::exit(1);
            }
        })
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
        let mut rng = XorShiftRng::seed_from_u64(seed);
        rng.fill_bytes(&mut original_buf[..]);
        Exerciser{
            file,
            file_size: 0,
            fname: cli.fname,
            good_buf,
            nomapread: cli.nomapread,
            nomapwrite: cli.nomapwrite,
            numops: cli.numops,
            opnum: cli.opnum,
            original_buf,
            rng,
            seed,
            steps: 0
        }
    }
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();
    let mut exerciser = Exerciser::from(cli);
    exerciser.exercise()
}
