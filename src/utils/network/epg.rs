use std::sync::Arc;
use log::debug;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::path::Path;

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::model::xmltv::{Epg, XmlTag};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigInput};
use crate::model::xmltv::TVGuide;
use crate::model::xmltv::{EPG_ATTRIB_ID, EPG_TAG_CHANNEL, EPG_TAG_PROGRAMME};
use crate::utils::file::file_utils::prepare_file_path;
use crate::utils::network::request;

/*
pub async fn get_xmltv(client: Arc<reqwest::Client>, _cfg: &Config, input: &ConfigInput, working_dir: &str) -> (Option<TVGuide>, Vec<M3uFilterError>) {
    match &input.epg_url {
        None => (None, vec![]),
        Some(_url) => {
            let Some(epg_url) = &input.epg_url else {
                return (None, vec![]);
            };
            
            let epg_urls = epg_url.to_vec();
            let Some(first_url) = epg_urls.first() else {
                return (None, vec![]);
            };
            
            debug!("Getting epg file path for url: {:?}", first_url);
            
            let persist_file_path = prepare_file_path(input.persist.as_deref(), working_dir, "")
                .map(|path| file_utils::add_prefix_to_filename(&path, "epg_", Some("xml")));
            
            match request::get_input_text_content_as_file(
                client,
                input,
                working_dir,
                first_url,
                persist_file_path,
            )
            .await {
                Ok(file) => (Some(TVGuide { file }), vec![]),
                Err(err) => (None, vec![err]),
            }
        }
    }
}
*/

pub async fn get_xmltvs(
    client: Arc<reqwest::Client>,
    _cfg: &Config,
    input: &ConfigInput,
    working_dir: &str,
) -> (Vec<TVGuide>, Vec<M3uFilterError>) {

    let Some(epg_urls) = &input.epg_url else {
        return (vec![], vec![]);
    };

    let urls = epg_urls.to_vec();
    let mut guides = vec![];
    let mut errors = vec![];

    for url in urls {
        debug!("Getting epg file path for url: {:?}", url);
    
        let hash = {
            let mut hasher = DefaultHasher::new();
            url.hash(&mut hasher);
            hasher.finish()
        };
    
        let mut path = PathBuf::from(working_dir);
        path.push(input.persist.as_deref().unwrap_or("input"));
    
        if let Err(e) = fs::create_dir_all(&path) {
            errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to create dir {:?}: {}", path, e)));
            continue;
        }
    
        path.push(format!("epg_{hash}.xml"));
        let persist_file_path = Some(path);
    
        match request::get_input_text_content_as_file(
            client.clone(),
            input,
            working_dir,
            &url,
            persist_file_path,
        )
        .await
        {
            Ok(file) => guides.push(TVGuide { file }),
            Err(err) => errors.push(err),
        }
    }

    (guides, errors)
}


pub fn parse_epg(path: &std::path::Path) -> Result<Epg, M3uFilterError> {
    let file = File::open(path).map_err(|e| {
        M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to parse EPG: {}", e))
        //M3uFilterError::new(M3uFilterErrorKind::IOError, format!("Failed to open EPG file: {}", e))
    })?;
    let reader = BufReader::new(file);

    XmlTag::parse_root(reader).map_err(|e| {
        M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to parse XMLTV: {}", e))
        //M3uFilterError::new(M3uFilterErrorKind::ParseError, format!("Failed to parse XMLTV: {}", e))
    })
}

pub fn parse_epgs(paths: &[PathBuf]) -> (Vec<Epg>, Vec<M3uFilterError>) {
    let mut results = vec![];
    let mut errors = vec![];

    for path in paths {
        match parse_epg(path) {
            Ok(epg) => results.push(epg),
            Err(err) => errors.push(err),
        }
    }

    (results, errors)
}

pub fn merge_epgs_dedup(epgs: Vec<Epg>) -> Epg {
    let mut seen_channel_ids = HashSet::new();
    let mut channels = Vec::new();
    let mut programmes = Vec::new();

    for epg in epgs {
        for tag in epg.children {
            match tag.name.as_str() {
                EPG_TAG_CHANNEL => {
                    if let Some(id) = tag.get_attribute_value(EPG_ATTRIB_ID) {
                        if seen_channel_ids.insert(id.clone()) {
                            channels.push(tag);
                        }
                    }
                }
                EPG_TAG_PROGRAMME => {
                    programmes.push(tag);
                }
                _ => {
                }
            }
        }
    }

    let mut all = Vec::new();
    all.extend(channels);
    all.extend(programmes);

    Epg {
        attributes: None,
        children: all,
    }
}
