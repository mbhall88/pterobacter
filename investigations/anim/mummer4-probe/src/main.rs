#[path = "../../probes/src/lib.rs"]
mod input;
use std::{env, ffi::CString, path::Path, process::ExitCode};

unsafe extern "C" {
    fn mummer4_pair_orientation(
        reference: *const std::ffi::c_char,
        query: *const std::ffi::c_char,
        prefix: *const std::ffi::c_char,
        threads: u32,
        error: *mut std::ffi::c_char,
        capacity: usize,
    ) -> i32;
}

fn align(reference: &str, query: &str, prefix: &str, threads: u32) -> Result<(), String> {
    let strings = [reference, query, prefix]
        .map(CString::new)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut error = [0_i8; 1024];
    // These owned C strings and the writable error buffer outlive the call.
    let code = unsafe {
        mummer4_pair_orientation(
            strings[0].as_ptr(),
            strings[1].as_ptr(),
            strings[2].as_ptr(),
            threads,
            error.as_mut_ptr(),
            error.len(),
        )
    };
    if code != 0 {
        let message = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) };
        return Err(message.to_string_lossy().into_owned());
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if !(4..=5).contains(&args.len()) {
        return Err("usage: probe REF QUERY PREFIX THREADS [concurrent]".into());
    }
    input::read_fasta(Path::new(&args[0]))?;
    input::read_fasta(Path::new(&args[1]))?;
    let threads = args[3].parse::<u32>().map_err(|e| e.to_string())?;
    if !(1..=4).contains(&threads) {
        return Err("development thread limit 1..4".into());
    }
    if args.get(4).is_some_and(|mode| mode == "concurrent") {
        if threads != 2 {
            return Err("concurrent mode: two independent objects, one thread each, total2".into());
        }
        std::thread::scope(|scope| {
            let duplicate = format!("{}.duplicate", args[2]);
            let borrowed = &args;
            let first = scope.spawn(|| align(&args[0], &args[1], &args[2], 1));
            let second = scope.spawn(move || align(&borrowed[0], &borrowed[1], &duplicate, 1));
            first.join().map_err(|_| "first bridge panicked")??;
            second.join().map_err(|_| "second bridge panicked")?
        })
    } else {
        align(&args[0], &args[1], &args[2], threads)
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
