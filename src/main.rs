#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use itertools::Itertools;
use log::LevelFilter;
use log::{error, info, trace};
use rayon::prelude::*;
use simple_logger::SimpleLogger;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use structopt::StructOpt;
use walkdir::WalkDir;

#[derive(Debug, StructOpt)]
#[structopt(about = "Compress folders sensibly")]
struct Opt {
	/// Increase verbosity
	#[structopt(short, long)]
	verbose: bool,

	/// Recurse
	#[structopt(short, long)]
	recursive: bool,

	/// Delete after
	#[structopt(short, long)]
	delete: bool,

	/// Extension
	#[structopt(short, long, default_value = "cbz")]
	extension: String,

	/// Folders to compress
	#[structopt(parse(from_os_str))]
	folders: Vec<PathBuf>,
}

fn main() {
	doit().unwrap();
}

fn doit() -> Result<(), Box<dyn Error>> {
	let options = Opt::from_args();

	let logger = SimpleLogger::new().with_colors(true).without_timestamps();
	if options.verbose {
		logger.with_level(LevelFilter::Trace).init().unwrap();
	} else {
		logger.with_level(LevelFilter::Info).init().unwrap();
	}

	let folders = if options.recursive {
		get_folders_recursively(options.folders.clone())?
	} else {
		options.folders.clone()
	};

	// unique-ify this, just in case the user provided duplicate folders
	let folders: Vec<PathBuf> = folders.into_iter().unique().collect();

	folders.par_iter().enumerate().for_each(|(i, folder)| {
		if let Err(err) = compress_folder(&folder, &options, i + 1, folders.len()) {
			error!("{}", err);
		};
	});

	info!("Done :D");

	Ok(())
}

fn get_folders_recursively(
	mut input_folders: Vec<PathBuf>,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
	let mut folders: Vec<PathBuf> = vec![];
	for folder in input_folders.drain(..) {
		if folder.exists() && folder.is_dir() {
			let child_folders = get_children(&folder)?;
			let child_folders = child_folders
				.iter()
				.filter(|child| child.is_dir())
				.collect::<Vec<&PathBuf>>();
			if child_folders.len() > 0 {
				for child in child_folders {
					folders.append(&mut get_folders_recursively(vec![child.clone()])?);
				}
			} else {
				folders.push(folder);
			}
		} else {
			// Push the bad folder. Handle error later on.
			folders.push(folder);
		}
	}
	Ok(folders)
}

fn compress_folder(
	folder: &PathBuf,
	options: &Opt,
	index: usize,
	total: usize,
) -> Result<(), Box<dyn Error>> {
	let folder_str = folder.to_str().ok_or("Bad path")?;
	if !folder.exists() {
		return Err(format!("{} doesn't exist", folder_str))?;
	}
	if !folder.is_dir() {
		return Err(format!("{} is not a directory", folder_str))?;
	}

	info!("[{} / {}] Starting \"{}\"...", index, total, &folder_str);

	let mut files = get_sorted_child_files(&folder, &options)?;

	let target = folder.as_path().with_extension(options.extension.clone());

	zip_folder(&mut files, &target)?;

	if options.delete {
		fs::remove_dir_all(folder)?;
	}

	info!("[{} / {}] Finished \"{}\"...", index, total, &folder_str);
	return Ok(());
}

fn get_children(folder: &PathBuf) -> Result<Vec<PathBuf>, Box<dyn Error>> {
	Ok(fs::read_dir(folder)?
		.filter_map(|entry| {
			if let Ok(entry) = entry {
				Some(entry.path())
			} else {
				None
			}
		})
		.collect::<Vec<PathBuf>>())
}

fn get_sorted_child_files(folder: &PathBuf, options: &Opt) -> Result<Vec<PathBuf>, Box<dyn Error>> {
	// let mut children = get_children(folder)?;
	let mut children = get_child_files_recursively(folder);
	children.sort_by(|a, b| split_and_pad(a).partial_cmp(&split_and_pad(b)).unwrap());
	return Ok(children);
}

fn get_child_files_recursively(folder: &PathBuf) -> Vec<PathBuf> {
	WalkDir::new(folder)
		.into_iter()
		.filter_map(|entry| {
			if let Ok(entry) = entry {
				// TODO: clean up this path/pathbuf nonsense
				let mut pathbuf = PathBuf::new();
				pathbuf.push(entry.path());
				Some(pathbuf)
			} else {
				None
			}
		})
		.collect::<Vec<PathBuf>>()
}

fn split_and_pad(input: &PathBuf) -> Vec<String> {
	let input = input.to_string_lossy();
	let mut pieces = vec![];
	let mut current: String = "".into();
	let mut is_digit = false;
	for c in input.chars() {
		if c.is_digit(10) != is_digit {
			if !current.is_empty() {
				// pad
				if is_digit {
					current = format!("{:0>5}", current);
				}
				pieces.push(current);
				current = "".into();
			}
			is_digit = !is_digit;
		}

		current.push(c);
	}

	if !current.is_empty() {
		pieces.push(current);
	}

	pieces
}

fn zip_folder(files: &mut Vec<PathBuf>, target: &PathBuf) -> Result<(), Box<dyn Error>> {
	let mut args = vec![target.clone()];
	args.append(files);
	let output = Command::new("zip")
		.args(args)
		.output()
		.expect("Failed to run 'zip'");

	// double-check we did this right
	if !output.status.success() || !target.exists() {
		return Err(format!(
			"Problem creating {}: {}",
			target.to_string_lossy(),
			output.status
		))?;
	}

	Ok(())
}
