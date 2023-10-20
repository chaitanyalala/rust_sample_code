//Parallel Grep-ish implementation -- Quite buggy
//
use std::error::Error;
use std::ffi::OsString;
use std::io::{self, BufReader, prelude::*};
use std::fs::{File, self};
use std::path::PathBuf;
use std::thread::{self, JoinHandle, available_parallelism};
use queues::*;
use std::collections::{HashSet, HashMap};
use colored::Colorize;

extern crate queues;


fn grep<R>(target: &str, reader: R, file_name: OsString) -> io::Result<()>
    where R: BufRead
{
    let mut line_number: u64 = 0;
    let file_name_str = file_name.into_string().unwrap();
    for line_result in reader.lines() {
        let line = line_result?;
        if line.contains(target) {
            let line_num = line_number.to_string().bright_cyan();
            println!("{}:{}: {}", file_name_str.bright_red(), line_num, line.bold());
        }
        line_number += 1;
    }
    Ok(())
}

fn grep_recursively(root: PathBuf, target: &String) {
    let num_cores = available_parallelism().unwrap().get() * 8;
    let mut paths : Queue<PathBuf> = Queue::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();

    let curr: &std::path::Path = root.as_path();
    match curr.try_exists() {
        Ok(_) => (),
        Err(_) => return,
    }
    let cannonical_path = fs::canonicalize(curr.clone()).unwrap();
    paths.add(cannonical_path.to_path_buf()).unwrap();

    let mut thread_ids : HashMap<i32, JoinHandle<()>> = HashMap::new();
    let mut id = 0;

    while paths.size() != 0 {
        let curr_elem = paths.remove().unwrap();
        let cannonical_path = fs::canonicalize(curr_elem.clone()).unwrap();
        if visited.contains(&cannonical_path) {
            // Been here, done that...
            continue;
        }
        // Add to visited
        visited.insert(cannonical_path.to_path_buf());

        if cannonical_path.is_file() {
            let file = cannonical_path.clone();

            // At this point we should fork a thread and grep the file.
            let t= target.clone();
            let file_name = file.clone().into_os_string();

            if thread_ids.len() < num_cores {
                let tid = thread::spawn(move || {
                    if let Ok(f) = File::open(file) {
                        let _ = grep(&t, BufReader::new(f), file_name);
                    }
                });
                thread_ids.insert(id, tid);
                id = id + 1;
            } else {
                // We are over our thread limit - let's do  inline for now
                if let Ok(f) = File::open(file) {
                    let _ = grep(&t, BufReader::new(f), file_name);
                }
            }

            // Wait for the rest later
            let mut to_remove = Vec::new();
            for (i, tid) in &thread_ids {
                if tid.is_finished() {
                    to_remove.push(i.to_owned());
                }
            }
            for k in to_remove.iter() {
                let res = thread_ids.remove(k);
                let _ = match res {
                    Some(t) => t.join(),
                    None => Ok({}),
                };
            }

            continue;
        }

        // Find all Children if curr_elem is a directory
        for child in fs::read_dir(cannonical_path.to_path_buf()).unwrap() {
            let entry = child.unwrap();
            let path = entry.path();
            if !path.exists() {
                continue;
            }
            let cpath = fs::canonicalize(path.clone()).unwrap();
            // Add to back of the queue
            paths.add(cpath.to_path_buf()).unwrap();
        }
    }
}

fn grep_main() -> Result<(), Box<dyn Error>> {
    // Get the command-line arguments. The first argument is the
    // string to search for; the rest are filenames.
    let mut args = std::env::args().skip(1);
    let target = match args.next() {
        Some(s) => s,
        None => Err("usage: grep PATTERN FILE...")?
    };
    let files: Vec<PathBuf> = args.map(PathBuf::from).collect();
    if files.is_empty() {
        panic!("No files specified");
    }
    /*
    Walk through all files in the list and see if any of them are directories
    If a file is indeed a dir, then recursively cd into that directory and add to the files
    Basically, do a BFS graph walk. There could be symlinks as well, so don't assume
    that it is a tree.
    */
    for f in &files {
        grep_recursively(f.to_path_buf(), &target);
    }
    Ok(())
}

fn main() {
    let result = grep_main();
    if let Err(err) = result {
        println!("{}", err);
        std::process::exit(1);
    }
}
