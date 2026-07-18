// Build scripts cannot return Result; panicking is the only error path.
#![expect(
    clippy::expect_used,
    reason = "build scripts have no recovery path — panic is correct"
)]

//! Build script for basilisk-stubs.
//!
//! Generates compile-time perfect-hash tables for the **offline baseline
//! only** — the small, loose, replaceable day-one fallback data. At runtime the
//! `python/typeshed` clone acquired and refreshed on the fly wholesale overrides
//! this baseline; these tables are consulted solely when no clone is available
//! (offline, clone failed, or pre-clone). See
//! `docs/plans/CHECKER-TYPESHED-RUNTIME-PLAN.md` and [STUBRES-TYPESHED-BASELINE].
//! Whether the baseline stays a compiled PHF table (as here) or moves to a loose
//! data file loaded at runtime is governed by the benchmark ratchet, not by this
//! script — see [CHKARCH-TESTING-BENCH-RATCHET].
//!
//! The generated tables:
//! * a `phf::Set` of all `CPython` 3.12 stdlib top-level module names from the
//!   typeshed VERSIONS data (replaces the hand-maintained `STDLIB_ROOTS` list
//!   in `e0010.rs`), and
//! * a `phf::Map` from third-party import roots to their typeshed stub
//!   distribution (`types-<dist>`), read from `data/typeshed_stub_distributions.tsv`.

use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Complete `CPython` 3.12 stdlib top-level module names.
///
/// Derived from typeshed's `stdlib/VERSIONS` file. Includes public modules,
/// standard internal modules, and deprecated-but-still-present modules.
/// Modules removed in 3.13+ are included since we target 3.12.
const STDLIB_MODULES: &[&str] = &[
    // ── Special modules ─────────────────────────────────────────────────
    "__future__",
    "__main__",
    "_thread",
    // ── A ────────────────────────────────────────────────────────────────
    "abc",
    "aifc",
    "argparse",
    "array",
    "ast",
    "asyncio",
    "atexit",
    "audioop",
    // ── B ────────────────────────────────────────────────────────────────
    "base64",
    "bdb",
    "binascii",
    "bisect",
    "builtins",
    "bz2",
    // ── C ────────────────────────────────────────────────────────────────
    "calendar",
    "cgi",
    "cgitb",
    "chunk",
    "cmath",
    "cmd",
    "code",
    "codecs",
    "codeop",
    "collections",
    "colorsys",
    "compileall",
    "concurrent",
    "configparser",
    "contextlib",
    "contextvars",
    "copy",
    "copyreg",
    "cProfile",
    "crypt",
    "csv",
    "ctypes",
    "curses",
    // ── D ────────────────────────────────────────────────────────────────
    "dataclasses",
    "datetime",
    "dbm",
    "decimal",
    "difflib",
    "dis",
    "distutils",
    "doctest",
    // ── E ────────────────────────────────────────────────────────────────
    "email",
    "encodings",
    "ensurepip",
    "enum",
    "errno",
    // ── F ────────────────────────────────────────────────────────────────
    "faulthandler",
    "fcntl",
    "filecmp",
    "fileinput",
    "fnmatch",
    "fractions",
    "ftplib",
    "functools",
    // ── G ────────────────────────────────────────────────────────────────
    "gc",
    "getopt",
    "getpass",
    "gettext",
    "glob",
    "graphlib",
    "grp",
    "gzip",
    // ── H ────────────────────────────────────────────────────────────────
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    // ── I ────────────────────────────────────────────────────────────────
    "idlelib",
    "imaplib",
    "imghdr",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    // ── J ────────────────────────────────────────────────────────────────
    "json",
    // ── K ────────────────────────────────────────────────────────────────
    "keyword",
    // ── L ────────────────────────────────────────────────────────────────
    "lib2to3",
    "linecache",
    "locale",
    "logging",
    "lzma",
    // ── M ────────────────────────────────────────────────────────────────
    "mailbox",
    "mailcap",
    "marshal",
    "math",
    "mimetypes",
    "mmap",
    "modulefinder",
    "multiprocessing",
    // ── N ────────────────────────────────────────────────────────────────
    "netrc",
    "nis",
    "nntplib",
    "numbers",
    // ── O ────────────────────────────────────────────────────────────────
    "operator",
    "optparse",
    "os",
    "ossaudiodev",
    // ── P ────────────────────────────────────────────────────────────────
    "pathlib",
    "pdb",
    "pickle",
    "pickletools",
    "pipes",
    "pkgutil",
    "platform",
    "plistlib",
    "poplib",
    "posix",
    "posixpath",
    "pprint",
    "profile",
    "pstats",
    "pty",
    "pwd",
    "py_compile",
    "pyclbr",
    "pydoc",
    // ── Q ────────────────────────────────────────────────────────────────
    "queue",
    "quopri",
    // ── R ────────────────────────────────────────────────────────────────
    "random",
    "re",
    "readline",
    "reprlib",
    "resource",
    "rlcompleter",
    "runpy",
    // ── S ────────────────────────────────────────────────────────────────
    "sched",
    "secrets",
    "select",
    "selectors",
    "shelve",
    "shlex",
    "shutil",
    "signal",
    "site",
    "smtpd",
    "smtplib",
    "sndhdr",
    "socket",
    "socketserver",
    "spwd",
    "sqlite3",
    "sre_compile",
    "sre_constants",
    "sre_parse",
    "ssl",
    "stat",
    "statistics",
    "string",
    "stringprep",
    "struct",
    "subprocess",
    "sunau",
    "symtable",
    "sys",
    "sysconfig",
    "syslog",
    // ── T ────────────────────────────────────────────────────────────────
    "tabnanny",
    "tarfile",
    "telnetlib",
    "tempfile",
    "termios",
    "test",
    "textwrap",
    "threading",
    "time",
    "timeit",
    "tkinter",
    "token",
    "tokenize",
    "tomllib",
    "trace",
    "traceback",
    "tracemalloc",
    "tty",
    "turtle",
    "turtledemo",
    "types",
    "typing",
    "typing_extensions",
    // ── U ────────────────────────────────────────────────────────────────
    "unicodedata",
    "unittest",
    "urllib",
    "uu",
    "uuid",
    // ── V ────────────────────────────────────────────────────────────────
    "venv",
    // ── W ────────────────────────────────────────────────────────────────
    "warnings",
    "wave",
    "weakref",
    "webbrowser",
    "winreg",
    "winsound",
    "wsgiref",
    // ── X ────────────────────────────────────────────────────────────────
    "xdrlib",
    "xml",
    "xmlrpc",
    // ── Z ────────────────────────────────────────────────────────────────
    "zipapp",
    "zipfile",
    "zipimport",
    "zlib",
    "zoneinfo",
    // ── Internal modules commonly imported ───────────────────────────────
    "_abc",
    "_bisect",
    "_codecs",
    "_collections_abc",
    "_csv",
    "_datetime",
    "_decimal",
    "_heapq",
    "_io",
    "_json",
    "_operator",
    "_signal",
    "_socket",
    "_struct",
    "_typing",
    "_contextvars",
    "_random",
    "_warnings",
    "_weakref",
    // ── Project module (always considered typed) ─────────────────────────
    "basilisk",
];

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    generate_stdlib_set(&out_dir);
    generate_stub_map(&out_dir);
}

/// Generate the `STDLIB_MODULES` perfect-hash set from the in-source list.
fn generate_stdlib_set(out_dir: &str) {
    let dest = Path::new(out_dir).join("stdlib_set.rs");
    let file = File::create(&dest).expect("failed to create stdlib_set.rs");
    let mut writer = BufWriter::new(file);

    write!(
        writer,
        "// Generated by build.rs — do not edit.\n\
         #[expect(clippy::unreadable_literal, reason = \"generated by phf_codegen\")]\n\
         /// Compile-time perfect hash set of `CPython` 3.12 stdlib top-level module names.\n\
         static STDLIB_MODULES: phf::Set<&'static str> = "
    )
    .expect("write failed");

    let mut builder = phf_codegen::Set::new();
    for module in STDLIB_MODULES {
        let _ = builder.entry(*module);
    }
    let built = builder.build();
    write!(writer, "{built}").expect("write failed");
    writeln!(writer, ";").expect("write failed");
}

/// Generate the `STUB_DISTRIBUTIONS` map (import root → `types-<dist>`) from the
/// committed typeshed data file.
fn generate_stub_map(out_dir: &str) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let data_path = Path::new(&manifest_dir)
        .join("data")
        .join("typeshed_stub_distributions.tsv");
    println!("cargo:rerun-if-changed={}", data_path.display());
    let data = fs::read_to_string(&data_path).expect("failed to read stub distribution data");

    let dest = Path::new(out_dir).join("stub_map.rs");
    let file = File::create(&dest).expect("failed to create stub_map.rs");
    let mut writer = BufWriter::new(file);

    write!(
        writer,
        "// Generated by build.rs — do not edit.\n\
         #[expect(clippy::unreadable_literal, reason = \"generated by phf_codegen\")]\n\
         /// Compile-time map from a Python import root to the typeshed stub\n\
         /// distribution that provides its types (`types-<dist>`).\n\
         static STUB_DISTRIBUTIONS: phf::Map<&'static str, &'static str> = "
    )
    .expect("write failed");

    let mut builder = phf_codegen::Map::new();
    // phf_codegen 0.13's `entry` borrows the value string for the builder's
    // lifetime, so the formatted distribution literals must outlive `build()`.
    let entries: Vec<(&str, String)> = data
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (import_root, distribution) = line
                .split_once('\t')
                .expect("stub distribution data line must be <import_root>\\t<distribution>");
            (import_root, format!("{distribution:?}"))
        })
        .collect();
    for (import_root, distribution) in &entries {
        let _ = builder.entry(*import_root, distribution);
    }
    let built = builder.build();
    write!(writer, "{built}").expect("write failed");
    writeln!(writer, ";").expect("write failed");
}
