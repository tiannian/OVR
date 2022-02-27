use nix::unistd::truncate;
use once_cell::sync::Lazy;
use ruc::*;
use std::{
    env,
    fs::{self, metadata, set_permissions, File, OpenOptions, Permissions},
    io::{self, prelude::*, BufReader, Read, Seek, SeekFrom},
    mem::size_of,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
};

const U64L: usize = size_of::<u64>();
const PAD_SIZE: usize = 256;

static SUFFIX: Lazy<u32> = Lazy::new(rand::random);
static OVRD_BIN: Lazy<String> = Lazy::new(|| format!("/tmp/ovrd_{}", *SUFFIX));
pub static TM_BIN: Lazy<String> = Lazy::new(|| format!("/tmp/tendermint_{}", *SUFFIX));

pub fn pack() -> Result<()> {
    let bin_path_orig = get_bin_path().c(d!())?;
    let bin_name = bin_path_orig.file_name().c(d!())?.to_str().c(d!())?;
    let bin_path = format!("/tmp/{}", bin_name);
    fs::copy(bin_path_orig, &bin_path).c(d!())?;

    let mut f = OpenOptions::new().append(true).open(bin_path).c(d!())?;
    let mut f_tendermint = File::open("tendermint").c(d!())?;
    let mut f_ovrd = File::open("ovrd").c(d!())?;

    f.write(&[0u8; PAD_SIZE][..]).c(d!())?;
    io::copy(&mut f_tendermint, &mut f).c(d!())?;
    f.write(&[0u8; PAD_SIZE][..]).c(d!())?;
    io::copy(&mut f_ovrd, &mut f).c(d!())?;
    f.write(&[0u8; PAD_SIZE][..]).c(d!())?;

    let tendermint_len = metadata("tendermint").c(d!())?.len();
    f.write(&tendermint_len.to_ne_bytes()[..]).c(d!())?;

    let ovrd_len = metadata("ovrd").c(d!())?.len();
    f.write(&ovrd_len.to_ne_bytes()[..]).c(d!()).map(|_| ())
}

pub fn unpack() -> Result<()> {
    let bin_path = get_bin_path().c(d!())?;

    let mut f = File::open(bin_path).c(d!())?;
    let mut tendermint_len = [0u8; U64L];
    let mut ovrd_len = [0u8; U64L];
    f.seek(SeekFrom::End(-2 * U64L as i64)).c(d!())?;
    f.read(&mut tendermint_len).c(d!())?;
    f.read(&mut ovrd_len).c(d!())?;
    let tendermint_len = u64::from_ne_bytes(tendermint_len) as usize;
    let ovrd_len = u64::from_ne_bytes(ovrd_len) as usize;

    let mut ovrd_reader = BufReader::with_capacity(ovrd_len, f);
    i64::try_from(ovrd_len + PAD_SIZE + 2 * U64L)
        .c(d!())
        .and_then(|siz| ovrd_reader.seek(SeekFrom::End(-siz)).c(d!()))
        .and_then(|_| ovrd_reader.fill_buf().c(d!()))?;
    let mut ovrd_writer = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(OVRD_BIN.as_str())
        .c(d!())?;
    io::copy(&mut ovrd_reader, &mut ovrd_writer)
        .c(d!())
        .and_then(|_| {
            set_permissions(OVRD_BIN.as_str(), Permissions::from_mode(0o755)).c(d!())
        })?;

    let mut tendermint_reader =
        BufReader::with_capacity(tendermint_len, ovrd_reader.into_inner());
    i64::try_from(tendermint_len + PAD_SIZE + ovrd_len + PAD_SIZE + 2 * U64L)
        .c(d!())
        .and_then(|siz| tendermint_reader.seek(SeekFrom::End(-siz)).c(d!()))
        .and_then(|_| tendermint_reader.fill_buf().c(d!()))?;
    let mut tendermint_writer = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(TM_BIN.as_str())
        .c(d!())?;
    io::copy(&mut tendermint_reader, &mut tendermint_writer)
        .c(d!())
        .and_then(|_| {
            set_permissions(TM_BIN.as_str(), Permissions::from_mode(0o755)).c(d!())
        })?;

    truncate(TM_BIN.as_str(), tendermint_len as i64).c(d!())
}

fn get_bin_path() -> Result<PathBuf> {
    let bin_path = env::current_exe().c(d!())?;
    let bin_size = metadata(&bin_path).c(d!())?.len() as usize;
    if (2 * U64L + 3 * PAD_SIZE) > bin_size {
        return Err(eg!("Invalid binary size"));
    }
    Ok(bin_path)
}
