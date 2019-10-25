use super::config::Config;
use super::format::{self, Extractor, Format, Files};
use super::epw::Epw;
use super::error::{LLResult, LLError};
use super::cse_result::CSEResult;
use super::consts::COMPONENT_SEARCH_ENGINE_URL;
use std::{
    path::PathBuf,
    collections::HashMap
};
use reqwest::{self, header};
use zip;

pub struct CSE {
    auth: String,
    config: Config
}

impl CSE {

    pub fn new(config: &Config) -> Self {
        CSE {
            auth: config.profile.to_base64(),
            config: config.clone()
        }
    }

    pub fn get(&self, epw: Epw) -> LLResult<CSEResult> {

        let id = epw.id;
        let url = format!("{base}{id}", base = COMPONENT_SEARCH_ENGINE_URL, id = id);

        let client = reqwest::Client::new();
        let req = client.get(&url).header(header::AUTHORIZATION, format!("Basic {auth}", auth = &self.auth));
        let mut res = req.send()?;

        let res_header = match res.headers().get("content-type") {
            Some(v) => v.to_str().unwrap_or("unknown"),
            None => "unknown"
        };

        if !res.status().is_success() {

            return Err(LLError::new(format!("Error downloading file: {}", res.status())))

        } else if res_header != "application/x-zip" {

            return Err(LLError::new("Error downloading file: Could not determine content type"))

        }

        let mut body = Vec::<u8>::new();
        if res.copy_to(&mut body).is_err() {
            return Err(LLError::new("Error copying data from response"))
        }

        let filename = match res.headers().get("content-disposition") {
            Some(v) => {
                let content_disposition = String::from_utf8_lossy(v.as_bytes()).to_string();
                content_disposition
                .replace("attachment;", "")
                .trim()
                .replace("filename=", "")
                .replace("\"", "")
                .trim()
                .to_string()
            },
            None => String::from("unknown.zip")
        };

        #[cfg(debug_assertions)]
        {
            println!("-- Debug info from {file}#{line} --", file = std::file!(), line = std::line!());
            println!("URL: {}", url);
            println!("Status: {}", res.status());
            println!("Headers {:#?}", res.headers());
            println!("Body length: {}", body.len());
            println!("Filename: {}", filename);
            println!("-- End debug info from {file}#{line} --", file = std::file!(), line = std::line!());
        }

        if &self.config.settings.format == &Format::ZIP {

            let mut files: Files = HashMap::new();
            files.insert(filename, body);

            Ok(CSEResult {
                output_path: self.config.settings.output_path.to_owned(),
                files: files
            })

        } else {

            let lib_name = match filename.starts_with("LIB_") {
                true => filename.as_str()[4..].replace(".zip", ""),
                false => filename.replace(".zip", "")
            };

            self.unzip(lib_name, body)

        }

    }

    fn unzip(&self, lib_name: String, data: Vec<u8>) -> LLResult<CSEResult> {

        let reader = std::io::Cursor::new(&data);
        let mut archive = zip::ZipArchive::new(reader)?;
        let mut files: Files = HashMap::new();

        for i in 0..archive.len() {

            let mut item = archive.by_index(i)?;
            let filename = String::from(item.name());

            match &self.config.settings.format {
                Format::EAGLE => format::eagle::Extractor::extract(&mut files, filename, &mut item)?,
                Format::EASYEDA => format::easyeda::Extractor::extract(&mut files, filename, &mut item)?,
                Format::KICAD => format::kicad::Extractor::extract(&mut files, filename, &mut item)?,
                Format::ZIP => return Err(LLError::new("This should be unreachable!"))
                // ! NOTE: DO NOT ADD A _ => {} CATCHER HERE!
            };

        }

        let path = PathBuf::from(&self.config.settings.output_path).join(lib_name);

        Ok(CSEResult {
            output_path: path.to_string_lossy().to_string(),
            files: files
        })

    }

}
