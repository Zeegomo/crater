use crate::common::CommandCraterExt;
use assert_cmd::prelude::*;
use difference::Changeset;
use rand::{self, distributions::Alphanumeric, Rng};
use serde_json::{self, Value};
use std::env;
use std::path::PathBuf;
use std::process::Command;

trait CommandMinicraterExt {
    fn minicrater_exec(&mut self);
}

impl CommandMinicraterExt for Command {
    fn minicrater_exec(&mut self) {
        if env::var_os("MINICRATER_SHOW_OUTPUT").is_some() {
            assert!(self.status().unwrap().success());
        } else {
            self.assert().success();
        }
    }
}

pub(super) struct MinicraterRun {
    pub(super) ex: &'static str,
    pub(super) crate_select: &'static str,
    pub(super) multithread: bool,
    pub(super) ignore_blacklist: bool,
    pub(super) mode: &'static str,
    pub(super) toolchains: &'static [&'static str],
}

impl Default for MinicraterRun {
    fn default() -> Self {
        MinicraterRun {
            ex: "default",
            crate_select: "demo",
            multithread: false,
            ignore_blacklist: false,
            mode: "build-and-test",
            toolchains: &["stable", "beta"],
        }
    }
}

impl MinicraterRun {
    pub(super) fn execute(&self) {
        let ex_dir = PathBuf::from("tests").join("minicrater").join(self.ex);
        let config_file = ex_dir.join("config.toml");
        let expected_file = ex_dir.join("results.expected.json");
        let actual_file = ex_dir.join("results.actual.json");

        let threads_count = if self.multithread { num_cpus::get() } else { 1 };

        let report_dir = tempfile::tempdir().expect("failed to create report dir");
        let ex_arg = format!(
            "--ex=minicrater-{}-{}",
            self.ex,
            rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(10)
                .collect::<String>()
        );

        // Create local list in the temp work dir
        Command::crater()
            .args(&["create-lists", "local"])
            .env("CRATER_CONFIG", &config_file)
            .minicrater_exec();

        // Define the experiment
        let mode = format!("--mode={}", self.mode);
        let crate_select = format!("--crate-select={}", self.crate_select);
        let mut define_args = vec!["define-ex", &ex_arg, &crate_select, &mode];
        define_args.extend(self.toolchains);
        if self.ignore_blacklist {
            define_args.push("--ignore-blacklist");
        }
        Command::crater()
            .args(&define_args)
            .env("CRATER_CONFIG", &config_file)
            .minicrater_exec();

        // Execute the experiment
        Command::crater()
            .args(&[
                "run-graph",
                &ex_arg,
                "--threads",
                &threads_count.to_string(),
            ])
            .args(if env::var_os("MINICRATER_FAST_WORKSPACE_INIT").is_some() {
                &["--fast-workspace-init"]
            } else {
                &[] as &[&str]
            })
            .env("CRATER_CONFIG", &config_file)
            .minicrater_exec();

        // Generate the report
        let mut cmd = Command::crater()
            .args(&["gen-report", &ex_arg])
            .env("CRATER_CONFIG", &config_file)
            .arg(report_dir.path());

        if env::var_os("MINICRATER_OUTPUT_TEMPLATE_CONTEXT").is_some() {
            cmd = cmd.arg("--output-templates");
        }

        cmd.minicrater_exec();

        // Read the JSON report
        let json_report = ::std::fs::read(report_dir.path().join("results.json"))
            .expect("failed to read json report");

        // Delete the experiment
        Command::crater()
            .args(&["delete-ex", &ex_arg])
            .env("CRATER_CONFIG", &config_file)
            .minicrater_exec();

        // Load the generated JSON report
        let parsed_report: Value =
            serde_json::from_slice(&json_report).expect("invalid json report");
        let mut actual_report = serde_json::to_vec_pretty(&parsed_report).unwrap();
        actual_report.push(b'\n');

        // Load the expected JSON report
        let expected_report = ::std::fs::read(&expected_file).unwrap_or(Vec::new());

        // Write the actual JSON report
        ::std::fs::write(&actual_file, &actual_report)
            .expect("failed to write copy of the json report");

        let changeset = Changeset::new(
            &String::from_utf8(expected_report)
                .expect("invalid utf-8 in the expected report")
                .replace("\r\n", "\n"),
            &String::from_utf8(actual_report).expect("invalid utf-8 in the actual report"),
            "\n",
        );
        if changeset.distance != 0 {
            eprintln!(
                "Difference between expected and actual reports:\n{}",
                changeset
            );
            eprintln!("To expect the new report in the future run:");
            eprintln!(
                "$ cp {} {}\n",
                actual_file.to_string_lossy(),
                expected_file.to_string_lossy()
            );
            panic!("invalid report generated by Crater");
        }
    }
}

#[macro_export]
macro_rules! minicrater {
    ($( $(#[$cfg:meta])* $name:ident $opts:tt,)*) => {
        $(
            #[test]
            #[ignore]
            $(#[$cfg])*
            fn $name() {
                use $crate::minicrater::driver::MinicraterRun;
                MinicraterRun $opts.execute();
            }
        )*
    }
}
