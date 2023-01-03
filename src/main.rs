// vim: tw=80
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    process
};

use log::{debug, error, info};
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

    fn doread(&mut self, offset: u64, size: usize ) {
        if size == 0 {
            debug!("{} skipping zero size read", self.steps);
            return;
        }
        if size as u64 + offset > self.file_size {
            debug!("{} skipping seek/read past EoF", self.steps);
            return;
        }
        info!("{} read {:#x} thru {:#x} ({:#x} bytes)", self.steps,
            offset,
            offset + size as u64,
            size);
        self.file.seek(SeekFrom::Start(offset)).unwrap();
        let mut temp_buf = vec![0u8; size];
        let read = self.file.read(&mut temp_buf[..]).unwrap();
        if read < size {
            error!("short read: {:#x} bytes instead of {:#x}", read, size);
            process::exit(1);
        }
        self.check_buffers(&temp_buf, offset)
    }

    fn dowrite(&mut self, offset: u64, size: usize) {
        if size == 0 {
            debug!("{} skipping zero size write", self.steps);
            return;
        }

        info!("{} write {:#x} thru {:#x} ({:#x} bytes)", self.steps,
            offset,
            offset + size as u64,
            size);

        self.gendata(offset, size);

        if self.file_size < offset + size as u64 {
            if self.file_size < offset {
                safemem::write_bytes(&mut self.good_buf[self.file_size as usize ..offset as usize], 0)
            }
            self.file_size = offset + size as u64;
        }
        self.file.seek(SeekFrom::Start(offset)).unwrap();
        let written = self.file.write(&self.good_buf[offset as usize..offset as usize + size]).unwrap();
        if written != size {
            error!("short write: {:#x} bytes instead of {:#x}", written, size);
            process::exit(1);
        }
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
                todo!()
            } else {
                self.dowrite(offset, size);
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
                self.doread(offset, size);
            }
        }
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
