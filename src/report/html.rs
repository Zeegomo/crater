use crate::assets;
use crate::experiments::Experiment;
use crate::prelude::*;
use crate::report::{
    analyzer::ReportCrates, archives::Archive, Color, Comparison, CrateResult, ReportWriter,
    ResultColor, ResultName, TestResults,
};
use crate::results::{EncodingType, FailureReason, TestResult};
#[cfg(feature = "minicrater")]
use crate::utils::serialize::{hashmap_deterministic_serialize, sort_vec};

use std::collections::HashMap;

#[cfg_attr(feature = "minicrater", derive(PartialEq, Eq, PartialOrd, Ord))]
#[derive(Serialize)]
struct NavbarItem {
    label: &'static str,
    url: &'static str,
    active: bool,
}

#[derive(PartialEq, Eq)]
enum CurrentPage {
    Summary,
    Full,
    Downloads,
}

#[cfg_attr(feature = "minicrater", derive(Eq))]
#[derive(Serialize)]
enum ReportCratesHTML {
    Plain(Vec<CrateResultHTML>),
    Tree {
        count: u32,
        #[cfg_attr(
            feature = "minicrater",
            serde(serialize_with = "hashmap_deterministic_serialize")
        )]
        tree: HashMap<String, Vec<CrateResultHTML>>,
    },
    RootResults {
        count: u32,
        #[cfg_attr(
            feature = "minicrater",
            serde(serialize_with = "hashmap_deterministic_serialize")
        )]
        results: HashMap<String, Vec<CrateResultHTML>>,
    },
}

impl CurrentPage {
    fn navbar(&self) -> Vec<NavbarItem> {
        vec![
            NavbarItem {
                label: "Summary",
                url: "index.html",
                active: *self == CurrentPage::Summary,
            },
            NavbarItem {
                label: "Full report",
                url: "full.html",
                active: *self == CurrentPage::Full,
            },
            NavbarItem {
                label: "Downloads",
                url: "downloads.html",
                active: *self == CurrentPage::Downloads,
            },
        ]
    }
}

// Some attention in the serializing details is needed when
// executing tests for minicrater. For this reason some fields
// are omitted because they contains non deterministic data
// and others are sorted even though we break internal consistency
#[derive(Serialize)]
struct ResultsContext<'a> {
    #[cfg_attr(feature = "minicrater", serde(skip_serializing), allow(dead_code))]
    ex: &'a Experiment,
    #[cfg_attr(feature = "minicrater", serde(serialize_with = "sort_vec"))]
    nav: Vec<NavbarItem>,
    #[cfg_attr(feature = "minicrater", serde(serialize_with = "sort_vec"))]
    categories: Vec<(Comparison, ReportCratesHTML)>,
    #[cfg_attr(
        feature = "minicrater",
        serde(serialize_with = "hashmap_deterministic_serialize")
    )]
    info: HashMap<Comparison, u32>,
    full: bool,
    crates_count: usize,
    #[cfg_attr(
        feature = "minicrater",
        serde(serialize_with = "hashmap_deterministic_serialize")
    )]
    comparison_colors: HashMap<Comparison, Color>,
    #[cfg_attr(feature = "minicrater", serde(serialize_with = "sort_vec"))]
    result_colors: Vec<Color>,
    #[cfg_attr(feature = "minicrater", serde(serialize_with = "sort_vec"))]
    result_names: Vec<String>,
}

#[derive(Serialize)]
struct DownloadsContext<'a> {
    ex: &'a Experiment,
    #[cfg_attr(feature = "minicrater", serde(serialize_with = "sort_vec"))]
    nav: Vec<NavbarItem>,
    crates_count: usize,
    #[cfg_attr(feature = "minicrater", serde(serialize_with = "sort_vec"))]
    available_archives: Vec<Archive>,
}

#[cfg_attr(feature = "minicrater", derive(PartialEq, Eq, PartialOrd, Ord))]
#[derive(Serialize)]
struct CrateResultHTML {
    name: String,
    url: String,
    res: Comparison,
    runs: [Option<BuildTestResultHTML>; 2],
}

// Map TestResult to usize to avoid the presence of special characters in html
#[cfg_attr(feature = "minicrater", derive(PartialEq, Eq, PartialOrd, Ord))]
#[derive(Serialize)]
struct BuildTestResultHTML {
    // The exact value of this field may change at runtime
    #[cfg_attr(feature = "minicrater", serde(skip_serializing))]
    res: usize,
    log: String,
}

fn write_report<W: ReportWriter>(
    ex: &Experiment,
    crates_count: usize,
    res: &TestResults,
    full: bool,
    to: &str,
    dest: &W,
    output_templates: bool,
) -> Fallible<()> {
    let mut comparison_colors = HashMap::new();
    let mut test_results_to_int = HashMap::new();
    let mut result_colors = Vec::new();
    let mut result_names = Vec::new();

    let mut to_html_crate_result = |result: CrateResult| {
        let mut runs = [None, None];

        for (pos, run) in result.runs.iter().enumerate() {
            if let Some(ref run) = run {
                let idx = test_results_to_int
                    .entry(run.res.clone())
                    .or_insert_with(|| {
                        result_colors.push(run.res.color());
                        result_names.push(run.res.name());
                        result_names.len() - 1
                    });
                runs[pos] = Some(BuildTestResultHTML {
                    res: *idx as usize,
                    log: run.log.clone(),
                });
            }
        }

        CrateResultHTML {
            name: result.name.clone(),
            url: result.url.clone(),
            res: result.res,
            runs,
        }
    };

    let categories = res
        .categories
        .iter()
        .filter(|(category, _)| full || category.show_in_summary())
        .map(|(&category, crates)| (category, crates.to_owned()))
        .flat_map(|(category, crates)| {
            comparison_colors.insert(category, category.color());

            match crates {
                ReportCrates::Plain(crates) => vec![(
                    category,
                    ReportCratesHTML::Plain(
                        crates
                            .into_iter()
                            .map(|result| to_html_crate_result(result))
                            .collect::<Vec<_>>(),
                    ),
                )]
                .into_iter(),
                ReportCrates::Complete { tree, results } => {
                    let tree = tree
                        .into_iter()
                        .map(|(root, deps)| {
                            (
                                root.to_string(),
                                deps.into_iter()
                                    .map(|result| to_html_crate_result(result))
                                    .collect::<Vec<_>>(),
                            )
                        })
                        .collect::<HashMap<_, _>>();
                    let results = results
                        .into_iter()
                        .map(|(res, krates)| {
                            (
                                if let TestResult::BuildFail(FailureReason::CompilerError(_)) = res
                                {
                                    res.to_string()
                                } else {
                                    res.name()
                                },
                                krates
                                    .into_iter()
                                    .map(|result| to_html_crate_result(result))
                                    .collect::<Vec<_>>(),
                            )
                        })
                        .collect::<HashMap<_, _>>();

                    vec![
                        (
                            category,
                            ReportCratesHTML::Tree {
                                count: tree.keys().len() as u32,
                                tree,
                            },
                        ),
                        (
                            category,
                            ReportCratesHTML::RootResults {
                                count: results.keys().len() as u32,
                                results,
                            },
                        ),
                    ]
                    .into_iter()
                }
            }
        })
        .collect();

    let context = ResultsContext {
        ex,
        nav: if full {
            CurrentPage::Full
        } else {
            CurrentPage::Summary
        }
        .navbar(),
        categories,
        info: res.info.clone(),
        full,
        crates_count,
        comparison_colors,
        result_colors,
        result_names,
    };

    info!("generating {}", to);
    if cfg!(feature = "minicrater") {
        dest.write_string(
            [to, ".context.json"].concat(),
            serde_json::to_string(&context)?.into(),
            &mime::APPLICATION_JSON,
        )?;
    } else {
        let html =
            minifier::html::minify(&assets::render_template("report/results.html", &context)?);
        dest.write_string(to, html.into(), &mime::TEXT_HTML)?;
    }

    Ok(())
}

fn write_downloads<W: ReportWriter>(
    ex: &Experiment,
    crates_count: usize,
    available_archives: Vec<Archive>,
    dest: &W,
    output_templates: bool,
) -> Fallible<()> {
    let context = DownloadsContext {
        ex,
        nav: CurrentPage::Downloads.navbar(),
        crates_count,
        available_archives,
    };

    info!("generating downloads.html");

    if cfg!(feature = "minicrater") {
        dest.write_string(
            "downloads.html.context.json",
            serde_json::to_string(&context)?.into(),
            &mime::APPLICATION_JSON,
        )?;
    } else {
        let html =
            minifier::html::minify(&assets::render_template("report/downloads.html", &context)?);
        dest.write_string("downloads.html", html.into(), &mime::TEXT_HTML)?;
    }

    Ok(())
}

pub fn write_html_report<W: ReportWriter>(
    ex: &Experiment,
    crates_count: usize,
    res: &TestResults,
    available_archives: Vec<Archive>,
    dest: &W,
    output_templates: bool,
) -> Fallible<()> {
    let js_in = assets::load("report.js")?;
    let css_in = assets::load("report.css")?;
    write_report(
        ex,
        crates_count,
        res,
        false,
        "index.html",
        dest,
        output_templates,
    )?;
    write_report(
        ex,
        crates_count,
        res,
        true,
        "full.html",
        dest,
        output_templates,
    )?;
    write_downloads(ex, crates_count, available_archives, dest, output_templates)?;

    info!("copying static assets");
    dest.write_bytes(
        "report.js",
        js_in.content()?.into_owned(),
        js_in.mime(),
        EncodingType::Plain,
    )?;
    dest.write_bytes(
        "report.css",
        css_in.content()?.into_owned(),
        css_in.mime(),
        EncodingType::Plain,
    )?;

    Ok(())
}

// All traits are implemented by hand to make sure they are consistent
#[cfg(feature = "minicrater")]
mod implement_test_traits {
    use super::ReportCratesHTML;
    use std::cmp::Ordering;

    impl PartialEq for ReportCratesHTML {
        fn eq(&self, other: &ReportCratesHTML) -> bool {
            serde_json::to_string(self)
                .unwrap()
                .eq(&serde_json::to_string(other).unwrap())
        }
    }

    impl PartialOrd for ReportCratesHTML {
        fn partial_cmp(&self, other: &ReportCratesHTML) -> Option<Ordering> {
            serde_json::to_string(self)
                .unwrap()
                .partial_cmp(&serde_json::to_string(other).unwrap())
        }
    }

    impl Ord for ReportCratesHTML {
        fn cmp(&self, other: &ReportCratesHTML) -> Ordering {
            serde_json::to_string(self)
                .unwrap()
                .cmp(&serde_json::to_string(other).unwrap())
        }
    }
}
