/*
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: 13/05/2020
Last Modified: 13/05/2020
License: MIT
*/

use crate::lidar::*;
use crate::tools::*;
use std;
use std::env;
use std::io::{Error, ErrorKind};
use std::path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

/// This tool can be used to convert one or more LAS files into the *zlidar* compressed LiDAR data format.
///
/// # See Also
/// `AsciiToLas`
pub struct ZlidarToLas {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl ZlidarToLas {
    pub fn new() -> ZlidarToLas {
        // public constructor
        let name = "ZlidarToLas".to_string();
        let toolbox = "LiDAR Tools".to_string();
        let description = "Converts one or more zlidar files into the LAS data format.".to_string();

        let mut parameters = vec![];
        parameters.push(ToolParameter {
            name: "Input ZLidar Files".to_owned(),
            flags: vec!["-i".to_owned(), "--inputs".to_owned()],
            description: "Input ZLidar files.".to_owned(),
            parameter_type: ParameterType::FileList(ParameterFileType::Lidar),
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Output Directory".to_owned(),
            flags: vec!["--outdir".to_owned()],
            description: "Output directory into which zlidar files are created. If unspecified, it is assumed to be the same as the inputs."
                .to_owned(),
            parameter_type: ParameterType::Directory,
            default_value: None,
            optional: true,
        });

        let sep: String = path::MAIN_SEPARATOR.to_string();
        let p = format!("{}", env::current_dir().unwrap().display());
        let e = format!("{}", env::current_exe().unwrap().display());
        let mut short_exe = e
            .replace(&p, "")
            .replace(".exe", "")
            .replace(".", "")
            .replace(&sep, "");
        if e.contains(".exe") {
            short_exe += ".exe";
        }
        let usage = format!(
            ">>.*{0} -r={1} -v --wd=\"*path*to*data*\" -i=\"file1.zlidar, file2.zlidar, file3.zlidar\"",
            short_exe, name
        )
        .replace("*", &sep);

        ZlidarToLas {
            name: name,
            description: description,
            toolbox: toolbox,
            parameters: parameters,
            example_usage: usage,
        }
    }
}

impl WhiteboxTool for ZlidarToLas {
    fn get_source_file(&self) -> String {
        String::from(file!())
    }

    fn get_tool_name(&self) -> String {
        self.name.clone()
    }

    fn get_tool_description(&self) -> String {
        self.description.clone()
    }

    fn get_tool_parameters(&self) -> String {
        let mut s = String::from("{\"parameters\": [");
        for i in 0..self.parameters.len() {
            if i < self.parameters.len() - 1 {
                s.push_str(&(self.parameters[i].to_string()));
                s.push_str(",");
            } else {
                s.push_str(&(self.parameters[i].to_string()));
            }
        }
        s.push_str("]}");
        s
    }

    fn get_example_usage(&self) -> String {
        self.example_usage.clone()
    }

    fn get_toolbox(&self) -> String {
        self.toolbox.clone()
    }

    fn run<'a>(
        &self,
        args: Vec<String>,
        working_directory: &'a str,
        verbose: bool,
    ) -> Result<(), Error> {
        let mut input_files: String = String::new();
        let mut output_directory: String = String::new();

        // read the arguments
        if args.len() == 0 {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Tool run with no parameters.",
            ));
        }
        for i in 0..args.len() {
            let mut arg = args[i].replace("\"", "");
            arg = arg.replace("\'", "");
            let cmd = arg.split("="); // in case an equals sign was used
            let vec = cmd.collect::<Vec<&str>>();
            let mut keyval = false;
            if vec.len() > 1 {
                keyval = true;
            }
            let flag_val = vec[0].to_lowercase().replace("--", "-");
            if flag_val == "-i" || flag_val == "-inputs" || flag_val == "-input" {
                if keyval {
                    input_files = vec[1].to_string();
                } else {
                    input_files = args[i + 1].to_string();
                }
            } else if flag_val == "-outdir" {
                output_directory = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            }
        }

        if verbose {
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
            println!("* Welcome to {} *", self.get_tool_name());
            println!("***************{}", "*".repeat(self.get_tool_name().len()));
        }

        let sep = std::path::MAIN_SEPARATOR;

        let start = Instant::now();

        if !output_directory.is_empty() && !output_directory.ends_with(sep) { 
            output_directory = format!("{}{}", output_directory, sep);
        }

        let mut cmd = input_files.split(";");
        let mut vec = cmd.collect::<Vec<&str>>().iter().map(|x| String::from(x.trim())).collect::<Vec<String>>();
        if vec.len() == 1 {
            cmd = input_files.split(",");
            vec = cmd.collect::<Vec<&str>>().iter().map(|x| String::from(x.trim())).collect::<Vec<String>>();
        }
        // let mut i = 1;
        let num_files = vec.len();
        let inputs = Arc::new(vec);
        let working_directory = Arc::new(working_directory.to_owned());
        let output_directory = Arc::new(output_directory.clone());
        let tile_list = Arc::new(Mutex::new(0..num_files));
        let num_procs = num_cpus::get() as isize;
        let (tx, rx) = mpsc::channel();
        for _ in 0..num_procs {
            let inputs = inputs.clone();
            let tile_list = tile_list.clone();
            let working_directory = working_directory.clone();
            let output_directory = output_directory.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let mut k = 0;
                let mut progress: usize;
                let mut old_progress: usize = 1;
                while k < num_files {
                    // Get the next tile up for examination
                    k = match tile_list.lock().unwrap().next() {
                        Some(val) => val,
                        None => break, // There are no more tiles to examine
                    };

                    let mut input_file = inputs[k].replace("\"", "").clone();
                    if !input_file.is_empty() {
                        if !input_file.contains(sep) && !input_file.contains("/") {
                            input_file = format!("{}{}", working_directory, input_file);
                        }

                        let input: LasFile = match LasFile::new(&input_file, "r") {
                            Ok(lf) => lf,
                            Err(_) => {
                                panic!(format!("Error reading file: {}", input_file));
                            }
                        };
        
                        let short_filename = input.get_short_filename();
                        let file_extension = get_file_extension(&input_file);
                        if file_extension.to_lowercase() != "zlidar" {
                            panic!("All input files should be of zlidar format.")
                        }
        
                        let output_file = if output_directory.is_empty() {
                            input_file.replace(&format!(".{}", file_extension), ".las")
                        } else {
                            format!("{}{}.las", output_directory, short_filename)
                        };
                        let mut output = LasFile::initialize_using_file(&output_file, &input);
        
                        let n_points = input.header.number_of_points as usize;
        
                        for p in 0..n_points {
                            let pr = input.get_record(p);
                            output.add_point_record(pr);
                            if verbose && num_files == 1 {
                                progress = (100.0_f64 * (p + 1) as f64 / (n_points - 1) as f64) as usize;
                                if progress != old_progress {
                                    println!("Creating output: {}%", progress);
                                    old_progress = progress;
                                }
                            }
                        }
                        let _ = match output.write() {
                            Ok(_) => {
                                // do nothing
                            }
                            Err(e) => println!("error while writing: {:?}", e),
                        };
                        tx.send(short_filename.clone()).unwrap();
                    } else {
                        tx.send(format!("Empty file name for tile {}.", k)).unwrap();
                    }
                }
            });
        }

        let mut progress: usize;
        let mut old_progress: usize = 1;
        for tile in 0..num_files {
            let file_nm = rx.recv().expect("Error receiving data from thread.");
            if !file_nm.contains("Empty") && num_files > 1 {
                println!("Completed conversion of {}", file_nm);
            } else {
                println!("{}", file_nm);
            }
            if verbose {
                progress = (100.0_f64 * tile as f64 / (num_files - 1) as f64) as usize;
                if progress != old_progress {
                    println!("Progress: {}%", progress);
                    old_progress = progress;
                }
            }
        }

        // let mut progress: usize;
        // let mut old_progress: usize = 1;

        // let start = Instant::now();

        // let mut cmd = input_files.split(";");
        // let mut vec = cmd.collect::<Vec<&str>>();
        // if vec.len() == 1 {
        //     cmd = input_files.split(",");
        //     vec = cmd.collect::<Vec<&str>>();
        // }
        // // let mut i = 1;
        // let num_files = vec.len();
        // for value in vec {
        //     if !value.trim().is_empty() {
        //         let mut input_file = value.trim().to_owned();
        //         if !input_file.contains(sep) && !input_file.contains("/") {
        //             input_file = format!("{}{}", working_directory, input_file);
        //         }

        //         let input: LasFile = match LasFile::new(&input_file, "r") {
        //             Ok(lf) => lf,
        //             Err(_) => {
        //                 return Err(Error::new(
        //                     ErrorKind::NotFound,
        //                     format!("Error reading file: {}", input_file),
        //                 ))
        //             }
        //         };

        //         let short_filename = input.get_short_filename();
        //         let file_extension = get_file_extension(&input_file);
        //         if file_extension.to_lowercase() != "zlidar" {
        //             return Err(Error::new(
        //                 ErrorKind::InvalidData,
        //                 "All input files should be of the ZLidar format.",
        //             ));
        //         }

        //         let output_file = if output_directory.is_empty() {
        //             input_file.replace(&format!(".{}", file_extension), ".zlidar")
        //         } else {
        //             format!("{}{}.zlidar", output_directory, short_filename)
        //         };
        //         let mut output = LasFile::initialize_using_file(&output_file, &input);

        //         let n_points = input.header.number_of_points as usize;

        //         for p in 0..n_points {
        //             let pr = input.get_record(p);
        //             output.add_point_record(pr);
        //             if verbose && num_files == 1 {
        //                 progress = (100.0_f64 * (p + 1) as f64 / (n_points - 1) as f64) as usize;
        //                 if progress != old_progress {
        //                     println!("Creating output: {}%", progress);
        //                     old_progress = progress;
        //                 }
        //             }
        //         }
                
        //         let _ = match output.write() {
        //             Ok(_) => {
        //                 if verbose {
        //                     if num_files == 1 {
        //                         println!("Complete!")
        //                     } else {
        //                         println!("Completed conversion of {}", short_filename);
        //                     }
        //                 }
        //             }
        //             Err(e) => println!("error while writing: {:?}", e),
        //         };
        //     }
        //     // i += 1;
        // }

        if verbose {
            let elapsed_time = get_formatted_elapsed_time(start);
            println!("{}", &format!("Elapsed Time: {}", elapsed_time));
        }

        Ok(())
    }
}

/// Returns the file extension.
pub fn get_file_extension(file_name: &str) -> String {
    let file_path = std::path::Path::new(file_name);
    let extension = file_path.extension().unwrap();
    let e = extension.to_str().unwrap();
    e.to_string()
}
