/*
This tool is part of the WhiteboxTools geospatial analysis library.
Authors: Dr. John Lindsay
Created: 22/06/2017
Last Modified: 13/10/2018
License: MIT
*/

use whitebox_raster::*;
use crate::tools::*;
use std::env;
use std::f64;
use std::io::{Error, ErrorKind};
use std::path;

/// This tool performs a weighted-sum overlay on multiple input raster images.
/// If you have a stack of rasters that you would like to sum, each with an
/// equal weighting (1.0), then use the `SumOverlay` tool instead.
///
/// # Warning
/// Each of the input rasters must have the same spatial extent and number of rows
/// and columns.
///
/// # See Also
/// `SumOverlay`
pub struct WeightedSum {
    name: String,
    description: String,
    toolbox: String,
    parameters: Vec<ToolParameter>,
    example_usage: String,
}

impl WeightedSum {
    pub fn new() -> WeightedSum {
        // public constructor
        let name = "WeightedSum".to_string();
        let toolbox = "GIS Analysis/Overlay Tools".to_string();
        let description =
            "Performs a weighted-sum overlay on multiple input raster images.".to_string();

        let mut parameters = vec![];
        parameters.push(ToolParameter {
            name: "Input Files".to_owned(),
            flags: vec!["-i".to_owned(), "--inputs".to_owned()],
            description: "Input raster files.".to_owned(),
            parameter_type: ParameterType::FileList(ParameterFileType::Raster),
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Weight Values (e.g. 1.7;3.5;1.2)".to_owned(),
            flags: vec!["-w".to_owned(), "--weights".to_owned()],
            description:
                "Weight values, contained in quotes and separated by commas or semicolons."
                    .to_owned(),
            parameter_type: ParameterType::String,
            default_value: None,
            optional: false,
        });

        parameters.push(ToolParameter {
            name: "Output File".to_owned(),
            flags: vec!["-o".to_owned(), "--output".to_owned()],
            description: "Output raster file.".to_owned(),
            parameter_type: ParameterType::NewFile(ParameterFileType::Raster),
            default_value: None,
            optional: false,
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
        let usage = format!(">>.*{} -r={} -v --wd='*path*to*data*' -i='image1.tif;image2.tif;image3.tif' --weights='0.3;0.2;0.5' -o=output.tif", short_exe, name).replace("*", &sep);

        WeightedSum {
            name: name,
            description: description,
            toolbox: toolbox,
            parameters: parameters,
            example_usage: usage,
        }
    }
}

impl WhiteboxTool for WeightedSum {
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
        match serde_json::to_string(&self.parameters) {
            Ok(json_str) => return format!("{{\"parameters\":{}}}", json_str),
            Err(err) => return format!("{:?}", err),
        }
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
        let mut input_files = String::new();
        let mut output_file = String::new();
        let mut weights_list = String::new();

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
            if flag_val == "-i" || flag_val == "-inputs" {
                input_files = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-o" || flag_val == "-output" {
                output_file = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            } else if flag_val == "-w" || flag_val == "-weights" {
                weights_list = if keyval {
                    vec[1].to_string()
                } else {
                    args[i + 1].to_string()
                };
            }
        }

        if verbose {
            let tool_name = self.get_tool_name();
            let welcome_len = format!("* Welcome to {} *", tool_name).len().max(28); 
            // 28 = length of the 'Powered by' by statement.
            println!("{}", "*".repeat(welcome_len));
            println!("* Welcome to {} {}*", tool_name, " ".repeat(welcome_len - 15 - tool_name.len()));
            println!("* Powered by WhiteboxTools {}*", " ".repeat(welcome_len - 28));
            println!("* www.whiteboxgeo.com {}*", " ".repeat(welcome_len - 23));
            println!("{}", "*".repeat(welcome_len));
        }

        let sep: String = path::MAIN_SEPARATOR.to_string();

        let mut progress: usize;
        let mut old_progress: usize = 1;

        if !output_file.contains(&sep) && !output_file.contains("/") {
            output_file = format!("{}{}", working_directory, output_file);
        }

        let mut cmd = input_files.split(";");
        let mut vec = cmd.collect::<Vec<&str>>();
        if vec.len() == 1 {
            cmd = input_files.split(",");
            vec = cmd.collect::<Vec<&str>>();
        }
        let num_files = vec.len();
        if num_files < 2 {
            return Err(Error::new(ErrorKind::InvalidInput,
                                "There is something incorrect about the input files. At least two inputs are required to operate this tool."));
        }

        let start = Instant::now();

        // Parse the weights list and convert it into numbers
        cmd = weights_list.split(";");
        let mut weights_str = cmd.collect::<Vec<&str>>();
        if vec.len() == 1 {
            cmd = weights_list.split(",");
            weights_str = cmd.collect::<Vec<&str>>();
        }
        let num_weights = weights_str.len();
        if num_weights != num_files {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "The number of weights specified must equal the number of input files.",
            ));
        }
        let mut weights = vec![];
        for w in weights_str {
            weights.push(w.to_string().parse::<f64>().unwrap());
        }

        // make sure that the weights sum to 1.0
        let mut weight_sum = 0.0f64;
        for i in 0..num_weights {
            weight_sum += weights[i];
        }
        for i in 0..num_weights {
            weights[i] /= weight_sum;
        }

        // We need to initialize output here, but in reality this can't be done
        // until we know the size of rows and columns, which occurs during the first loop.
        let mut output: Raster = Raster::new(&output_file, "w")?;
        let mut rows = 0isize;
        let mut columns = 0isize;
        let mut in_nodata: f64;
        let mut out_nodata: f64 = -32768.0f64;
        let mut in_val: f64;
        let mut read_first_file = false;
        let mut i = 1;
        let mut j = 0usize;
        for value in vec {
            if !value.trim().is_empty() {
                if verbose {
                    println!("Reading data...")
                };

                let mut input_file = value.trim().to_owned();
                if !input_file.contains(&sep) && !input_file.contains("/") {
                    input_file = format!("{}{}", working_directory, input_file);
                }
                let input = Raster::new(&input_file, "r")?;
                in_nodata = input.configs.nodata;
                if !read_first_file {
                    read_first_file = true;
                    rows = input.configs.rows as isize;
                    columns = input.configs.columns as isize;
                    out_nodata = in_nodata;

                    // initialize the output file and low_val
                    output = Raster::initialize_using_file(&output_file, &input);
                    output.reinitialize_values(0.0);
                }
                // check to ensure that all inputs have the same rows and columns
                if input.configs.rows as isize != rows || input.configs.columns as isize != columns
                {
                    return Err(Error::new(ErrorKind::InvalidInput,
                                "The input files must have the same number of rows and columns and spatial extent."));
                }

                for row in 0..rows {
                    for col in 0..columns {
                        if output[(row, col)] != out_nodata {
                            in_val = input[(row, col)];
                            if in_val != in_nodata {
                                output.increment(row, col, in_val * weights[j]);
                            } else {
                                output[(row, col)] = out_nodata;
                            }
                        }
                    }
                    if verbose {
                        progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
                        if progress != old_progress {
                            println!("Progress (loop {} of {}): {}%", i, num_files, progress);
                            old_progress = progress;
                        }
                    }
                }
            }
            i += 1;
            j += 1;
        }

        let elapsed_time = get_formatted_elapsed_time(start);
        output.add_metadata_entry(format!(
            "Created by whitebox_tools\' {} tool",
            self.get_tool_name()
        ));
        output.add_metadata_entry(format!("Elapsed Time (including I/O): {}", elapsed_time));

        if verbose {
            println!("Saving data...")
        };
        let _ = match output.write() {
            Ok(_) => {
                if verbose {
                    println!("Output file written")
                }
            }
            Err(e) => return Err(e),
        };

        if verbose {
            println!(
                "{}",
                &format!("Elapsed Time (including I/O): {}", elapsed_time)
            );
        }

        Ok(())
    }
}
