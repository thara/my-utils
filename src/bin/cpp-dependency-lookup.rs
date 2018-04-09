use std::env;
use std::fs::{self, DirEntry, File};
use std::io::{self, stdout, BufReader, BufWriter};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

#[macro_use]
extern crate error_chain;

extern crate regex;
use regex::Regex;

#[macro_use]
extern crate lazy_static;

mod errors {
    error_chain!{
        foreign_links {
            Io(::std::io::Error);
        }
    }
}
use errors::*;

quick_main!(run);

fn run() -> Result<()> {
    let mut argv = env::args();
    let program = argv.next().unwrap();

    let depender_dir = argv.next().ok_or(format!(
        "Usage: {} {{depender dir path}} {{depender dir path}}",
        program
    ))?;
    let dependee_dir = argv.next().ok_or(format!(
        "Usage: {} {{depender dir path}} {{dependee dir path}}",
        program
    ))?;

    if depender_dir == dependee_dir {
        return Err(
            "Can not use same directories as depender and dependee".into(),
        );
    }

    if fs::metadata(&depender_dir).is_err() {
        return Err(format!("{} does not exists.", &depender_dir).into());
    }
    if fs::metadata(&dependee_dir).is_err() {
        return Err(format!("{} does not exists.", &dependee_dir).into());
    }

    let depender_path = Path::new(&depender_dir);

    fn collect_deps(entry: &DirEntry, out: &mut DependencyMap) -> io::Result<()> {
        let path: String = entry.path().to_str().unwrap().into();
        let f = File::open(path.clone())?;
        let br = BufReader::new(f);
        for l in br.lines().filter_map(|v| v.ok()) {
            if l.starts_with("#include") {
                lazy_static! {
                    static ref RE: Regex = Regex::new("\"(.*)\"").unwrap();
                }
                if let Some(cap) = RE.captures_iter(&l).next() {
                    let depender = entry.path().to_str().unwrap().into();
                    let dependee = (&cap[1]).into();
                    out.entry(dependee).or_insert_with(|| Vec::new()).push(
                        depender,
                    );
                }
            }
        }
        Ok(())
    }

    let mut deps: DependencyMap = HashMap::new();
    visit_dirs(&depender_path, &collect_deps, &mut deps)?;

    let filter_deps = |entry: &DirEntry, out: &mut Vec<Dependee>| -> io::Result<()> {
        let path = entry
            .path()
            .file_name()
            .and_then(|p| p.to_str())
            .expect("filename")
            .into();
        if (&deps).contains_key(&path) {
            let dep = Dependee{ header: path, path: entry.path().to_owned() };
            out.push(dep);
        }
        Ok(())
    };

    let dependee_path = Path::new(&dependee_dir);
    let mut dependees = Vec::new();
    visit_dirs(&dependee_path, &filter_deps, &mut dependees)?;

    let out = stdout();
    let mut out = BufWriter::new(out.lock());

    let mut results : Vec<(String, String)>= Vec::new();
    for dependee in dependees {
        let header = &dependee.header;
        for v in deps.get(header).expect(header) {
            let depender = Path::new(&v)
                .strip_prefix(&depender_dir)
                .and_then(|p| Ok(p.to_str().expect("depender str")))
                .expect(&v);
            let path = dependee_path.join(&dependee.path);
            let path = path.strip_prefix(&dependee_dir).and_then(|p| Ok(p.to_str().unwrap())).unwrap();
            results.push((depender.into(), path.into()));
        }
    }
    results.sort_by(|a, b| (a.0).cmp(&b.0));

    for (depender, path) in results {
        writeln!(out, "{} => {}", &depender, path).unwrap();
    }

    Ok(())
}

struct Dependee {
    header: String,
    path: PathBuf,
}

type DependencyMap = HashMap<String, Vec<String>>;

// one possible implementation of walking a directory only visiting files
fn visit_dirs<V, F>(dir: &Path, cb: &F, out: &mut V) -> io::Result<()>
where
    F: Fn(&DirEntry, &mut V) -> io::Result<()>,
{
    if dir.is_dir() {
        for entry in try!(fs::read_dir(dir)) {
            let entry = try!(entry);
            let path = entry.path();
            if path.is_dir() {
                try!(visit_dirs(&path, cb, out));
            } else {
                cb(&entry, out)?;
            }
        }
    }
    Ok(())
}
