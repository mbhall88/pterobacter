use anim_feasibility_probes::read_fasta;
use std::{
    env,
    ffi::{CString, c_char, c_int},
    fs::File,
    path::Path,
    process::{self, Command, Stdio},
};

unsafe extern "C" {
    fn mummer_stage(stage: *const c_char, argc: c_int, argv: *mut *mut c_char) -> c_int;
}
fn stage(
    name: &str,
    args: &[String],
    input: Option<File>,
    output: Option<File>,
) -> Result<(), String> {
    let mut child = Command::new(env::current_exe().map_err(|e| e.to_string())?);
    child.arg("--stage").arg(name).args(args);
    child.stdin(input.map_or_else(Stdio::null, Stdio::from));
    child.stdout(output.map_or_else(Stdio::null, Stdio::from));
    let status = child.status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("stage {name} failed: {status}"));
    }
    Ok(())
}
fn file(path: &str, create: bool) -> Result<File, String> {
    (if create {
        File::create(path)
    } else {
        File::open(path)
    })
    .map_err(|e| format!("{path}: {e}"))
}
fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 4 {
        return Err("usage: mummer3-probe REFERENCE QUERY PREFIX".into());
    }
    let (reference, query, prefix) = (&args[1], &args[2], &args[3]);
    read_fasta(Path::new(reference))?;
    read_fasta(Path::new(query))?;
    let ntref = format!("{prefix}.ntref");
    let matches = format!("{prefix}.matches");
    let clusters = format!("{prefix}.mgaps");
    stage(
        "prenuc",
        std::slice::from_ref(reference),
        None,
        Some(file(&ntref, true)?),
    )?;
    stage(
        "mummer",
        &[
            "-mum".into(),
            "-b".into(),
            "-l".into(),
            "20".into(),
            "-n".into(),
            ntref,
            query.clone(),
        ],
        None,
        Some(file(&matches, true)?),
    )?;
    stage(
        "mgaps",
        &[
            "-l".into(),
            "65".into(),
            "-s".into(),
            "90".into(),
            "-d".into(),
            "5".into(),
            "-f".into(),
            ".12".into(),
        ],
        Some(file(&matches, false)?),
        Some(file(&clusters, true)?),
    )?;
    stage(
        "postnuc",
        &[
            "-b".into(),
            "200".into(),
            reference.clone(),
            query.clone(),
            prefix.clone(),
        ],
        Some(file(&clusters, false)?),
        None,
    )?;
    stage(
        "delta-filter",
        &["-1".into(), format!("{prefix}.delta")],
        None,
        Some(file(&format!("{prefix}.filter"), true)?),
    )?;
    Ok(())
}
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|s| s == "--stage") {
        let name = CString::new(args.get(2).expect("stage name").as_str()).unwrap();
        let strings: Vec<CString> = args[2..]
            .iter()
            .map(|s| CString::new(s.as_str()).unwrap())
            .collect();
        let mut pointers: Vec<*mut c_char> =
            strings.iter().map(|s| s.as_ptr().cast_mut()).collect();
        let argc = pointers.len() as c_int;
        pointers.push(std::ptr::null_mut());
        // Each stage has fresh process state; upstream exit() cannot kill the driver.
        let code = unsafe { mummer_stage(name.as_ptr(), argc, pointers.as_mut_ptr()) };
        process::exit(code);
    }
    if let Err(error) = run(&args) {
        eprintln!("{error}");
        process::exit(2);
    }
}
