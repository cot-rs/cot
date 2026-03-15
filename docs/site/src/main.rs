use std::time::Duration;

use cot::cli::CliMetadata;
use cot::config::{ProjectConfig, StaticFilesConfig, StaticFilesPathRewriteMode};
use cot::error::handler::DynErrorPageHandler;
use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandler, RootHandlerBuilder};
use cot::static_files::StaticFilesMiddleware;
use cot::{AppBuilder, Project};
use cot_site::{CotSiteApp, cot_site_common, cot_site_handle_error, md_page};

struct CotSiteProject;

impl Project for CotSiteProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        // we don't need to load any config
        Ok(ProjectConfig::builder()
            .static_files(
                StaticFilesConfig::builder()
                    .url("/")
                    .rewrite(StaticFilesPathRewriteMode::QueryParam)
                    .cache_timeout(Duration::from_secs(365 * 24 * 60 * 60))
                    .build(),
            )
            .build())
    }

    fn register_apps(&self, modules: &mut AppBuilder, _app_context: &RegisterAppsContext) {
        modules.register_with_views(
            CotSiteApp::new(vec![
                (
                    "Getting started",
                    vec![
                        md_page!("introduction"),
                        md_page!("templates"),
                        md_page!("forms"),
                        md_page!("db-models"),
                        md_page!("admin-panel"),
                        md_page!("static-files"),
                        md_page!("sending-emails"),
                        md_page!("caching"),
                        md_page!("error-pages"),
                        md_page!("openapi"),
                        md_page!("testing"),
                    ],
                ),
                ("Upgrading", vec![md_page!("upgrade-guide")]),
                ("About", vec![md_page!("framework-comparison")]),
            ]),
            "",
        );
    }

    fn middlewares(&self, handler: RootHandlerBuilder, context: &MiddlewareContext) -> RootHandler {
        let handler = handler.middleware(StaticFilesMiddleware::from_context(context));
        #[cfg(debug_assertions)]
        let handler = handler.middleware(cot::middleware::LiveReloadMiddleware::new());
        handler.build()
    }

    fn error_handler(&self) -> DynErrorPageHandler {
        DynErrorPageHandler::new(cot_site_handle_error)
    }
}

#[cot::main]
fn main() -> impl Project {
    CotSiteProject
}
