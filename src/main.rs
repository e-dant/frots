use anyhow::anyhow;
use anyhow::Result;
use byte_unit::Byte;
use clap::arg;
use clap::Parser;
use log::error;
use log::info;
use std::fs::rename;
use std::fs::File;
use std::io::stdin;
use std::io::Write;

#[derive(Parser)]
#[clap(long_about = r#"
The only file rotation tool I'm aware of that *actually handles stdout*.

`logrotate` (which you really ought to use if you can) happens to use `rename()`
under the hood to move the primary logfile into rotation, which confuses programs
already writing to it (they will now be writing to the rotated file, which is weird).
`frots` was primarily made to solve that problem.

Reads standard input into the specified `file` (`-f`) until:
- Standard input reaches EOF. This is the normal, and we exit normally.
- Unrecoverable errors occur. In this case, we display an error message
  and exit with a returncode of 1.

If `file` grows to reach or exceed the `limit` (`-s`), then:
1. Synchronize `file` with the disk.
2. In the range 1..`R` (where `R` is `-r`, `--num-rotate`),
   rename `file.N` to `file.N+1` if `N < R`. (I.e., rotate the files.)
3. Rename `file` `file.1`
4. Create or open the specified file and continue writing to it.

Example usage:
```sh
# Two files (one active, one rotated) of 1GB each; Verbose output along with "tee"ing
some-prog | frots -f /var/log/prog/a.log -s 1G -r 2 --tee -v
```

Notes:
- "Rename" file operations mean "in place" renaming, as-if with `rename()`, not copy-and-move.
- "Synchronize" file operations mean to-disk synchronization, as-if with `fsync()`.
"#)]
struct Cli {
    /// The name of the file to write to
    #[arg(short = 'f', long)]
    file_path: String,
    /// The size limit of the file in a byte-unit format (1KB, 3M, 4G, etc.) before rotation
    #[arg(short = 's', long)]
    file_sz_lim: String,
    /// The number of files to rotate through (see '--help' for more), must be >= 1
    #[arg(short = 'r', long, default_value = "1")]
    num_rotate: u16,
    /// If "b" means bytes or bits for the `file_sz_lim`, i.e., if 1Kb = 8192b or 1024B
    #[arg(long, default_value = "false")]
    b_is_bits: bool,
    /// Whether to "tee" stdin to stdout as well as to `file` (just like `tee(1)`)
    #[arg(long, default_value = "false")]
    tee: bool,
    /// Verbose output, as-if by setting `RUST_LOG=info` in the environment
    #[arg(short = 'v', long)]
    verbose: bool,
}

fn rot_nr_scheme(num_rotate: u16) -> impl Iterator<Item = (u16, u16)> {
    let xs = 0..num_rotate;
    let ys = 1..=num_rotate;
    xs.rev().zip(ys.rev()).filter(move |(_, y)| *y < num_rotate)
}

fn rot_file_scheme(
    file_path: &str,
    num_rotate: u16,
) -> impl Iterator<Item = (String, String)> + '_ {
    let w_file_path = move |(from, to)| {
        let from = match from {
            0 => format!("{file_path}"),
            n => format!("{file_path}.{n}"),
        };
        let to = format!("{file_path}.{to}");
        (from, to)
    };
    rot_nr_scheme(num_rotate).map(w_file_path)
}

fn rot(file_path: &str, num_rotate: u16) -> Result<File> {
    for (from, to) in rot_file_scheme(file_path, num_rotate) {
        info!("Renaming {from} -> {to}");
        rename(&from, &to)?;
    }
    Ok(File::create(file_path)?)
}

fn main() -> Result<()> {
    let args = Cli::parse();
    if args.verbose {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
    let file_sz_lim: usize = Byte::parse_str(args.file_sz_lim, args.b_is_bits)?.try_into()?;
    let num_rotate = match args.num_rotate {
        0 => return Err(anyhow!("`num_rotate` must be >= 1")),
        n => n,
    };
    let file_path = &args.file_path; // Or we write &args.file_path a lot and fmts are weird.
    let mut file = File::create(file_path)?;
    let mut file_sz = 0;
    let mut buf = String::with_capacity(4096);
    info!(
        "Rotation scheme: [(from, to),...] {:?}",
        rot_file_scheme(file_path, num_rotate).collect::<Vec<_>>()
    );
    if file.metadata()?.len() as usize > file_sz_lim {
        info!("Rotating (initial sz >= lim={file_sz_lim}, R={num_rotate})");
        rot(file_path, num_rotate)?;
    }
    loop {
        let n = stdin().read_line(&mut buf)?;
        file_sz += n;
        if n == 0 {
            return Ok(()); // EOF
        }
        if args.tee {
            print!("{buf}");
        }
        if file_sz >= file_sz_lim {
            info!("Rotating (sz={file_sz} >= lim={file_sz_lim}, R={num_rotate})");
            file_sz = 0;
            if let Err(e) = file.sync_all() {
                error!("Error syncing file w/ disk: {e}");
            }
            file = rot(file_path, num_rotate)?;
        }
        file.write_all(buf.as_bytes())?;
        buf.clear();
    }
}

#[test]
#[cfg(test)]
fn test_rot() {
    assert_eq!(rot_nr_scheme(0).collect::<Vec<_>>(), vec![]);
    assert_eq!(rot_nr_scheme(1).collect::<Vec<_>>(), vec![]);
    assert_eq!(rot_nr_scheme(2).collect::<Vec<_>>(), vec![(0, 1)]);
    assert_eq!(rot_nr_scheme(3).collect::<Vec<_>>(), vec![(1, 2), (0, 1)]);
    assert_eq!(rot_file_scheme("f", 0).collect::<Vec<_>>(), vec![]);
    assert_eq!(rot_file_scheme("f", 1).collect::<Vec<_>>(), vec![]);
    assert_eq!(
        rot_file_scheme("f", 2).collect::<Vec<_>>(),
        vec![("f".into(), "f.1".into())]
    );
    assert_eq!(
        rot_file_scheme("f", 3).collect::<Vec<_>>(),
        vec![("f.1".into(), "f.2".into()), ("f".into(), "f.1".into())]
    );
}
