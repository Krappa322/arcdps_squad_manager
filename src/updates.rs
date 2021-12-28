use std::{
    fs::{self, File},
    io::{Read, Write},
    mem::swap,
    path::PathBuf,
};

use serde::Deserialize;
use version_compare::Version;
use winapi::{
    shared::minwindef::{HINSTANCE, MAX_PATH},
    um::{errhandlingapi::GetLastError, libloaderapi::GetModuleFileNameW},
};

use crate::NEW_UPDATE;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Release {
    pub prerelease: bool,
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug)]
pub enum UpdateStatus {
    UpdateAvailable(String),
    Downloading,
    Updating,
    RestartPending,
    UpdateError(String),
}

pub struct UpdateInfo {
    pub newer_release: Release,
    pub status: UpdateStatus,
}

impl UpdateInfo {
    pub fn new(newer_release: Release, download_url: String) -> Self {
        Self {
            newer_release,
            status: UpdateStatus::UpdateAvailable(download_url),
        }
    }
}

const RELEASE_REPO: &str = "Krappa322/arcdps_unofficial_extras_releases";

pub fn tag_to_version_num(pTagName: &str) -> String {
    pTagName
        .chars()
        .filter(|c| c.is_numeric() || c == &'.')
        .collect()
}

pub fn install_update(pUpdate: &mut UpdateInfo) {
    let download_url: String;
    match pUpdate.status {
        UpdateStatus::UpdateAvailable(_) => {}
        _ => {
            debug!(
                "Dropping update request since update is already in progress or failed {:?}",
                pUpdate.status
            );
            return;
        }
    }

    let mut swap_value = UpdateStatus::Downloading;
    swap(&mut pUpdate.status, &mut swap_value);

    match swap_value {
        UpdateStatus::UpdateAvailable(x) => download_url = x,
        _ => unreachable!(),
    }

    std::thread::spawn(move || {
        debug!("Fetching {:?}", &download_url);
        let data = match download_data(&download_url) {
            Some(x) => x,
            None => {
                warn!("Downloading data failed");
                if let Some(x) = NEW_UPDATE.write().as_mut() {
                    x.status = UpdateStatus::UpdateError("Failed to download update".to_string());
                }
                return;
            }
        };

        if let Some(x) = NEW_UPDATE.write().as_mut() {
            x.status = UpdateStatus::Updating;
        }

        if replace_binary(data) == false {
            warn!("Replacing binary failed");
            if let Some(x) = NEW_UPDATE.write().as_mut() {
                x.status = UpdateStatus::UpdateError("Failed to apply update".to_string());
            }
            return;
        };

        info!("Successfully updated to {:?}", &download_url);
        if let Some(x) = NEW_UPDATE.write().as_mut() {
            x.status = UpdateStatus::RestartPending;
        }
    });
}

fn download_data(pDownloadUrl: &String) -> Option<Vec<u8>> {
    let response = match ureq::get(&pDownloadUrl).call() {
        Ok(x) => x,
        Err(e) => {
            warn!("Getting response failed {}", e);
            return None;
        }
    };

    let len_header = match response.header("Content-Length") {
        Some(x) => x,
        None => {
            warn!("Didn't get Content-Length header in response");
            return None;
        }
    };

    let len: u64 = match len_header.parse() {
        Ok(x) => x,
        Err(e) => {
            warn!(
                "Content-Length header ({:?}) is not an integer - {}",
                len_header, e
            );
            return None;
        }
    };

    if len > 128_000_000 {
        warn!("Got too large Content-Length {} - malicious server?", len);
        return None;
    }

    // TODO: Is there any way to pipe this stream straight into a file rather than making a full copy in memory of it
    // like this? Or maybe copy it in a chunked fashion to reduce the required buffer size
    let mut bytes: Vec<u8> = Vec::with_capacity(len as usize);
    match response.into_reader().take(len).read_to_end(&mut bytes) {
        Ok(_) => {}
        Err(e) => {
            warn!("Reading content failed {}", e);
            return None;
        }
    };

    Some(bytes)
}

fn replace_binary(pData: Vec<u8>) -> bool {
    let self_path_str = unsafe {
        extern "C" {
            pub static __ImageBase: u8;
        }
        let module_base: *const u8 = &__ImageBase;

        let mut self_path_str = std::mem::zeroed::<[u16; MAX_PATH]>();
        if GetModuleFileNameW(
            module_base as HINSTANCE,
            self_path_str.as_mut_ptr(),
            self_path_str.len() as u32,
        ) == 0
        {
            warn!(
                "GetModuleFileNameW on ImageBase {:?} failed - {}",
                module_base,
                GetLastError()
            );
            return false;
        }

        self_path_str
    };

    let self_path_str = String::from_utf16_lossy(
        &self_path_str
            .iter()
            .take_while(|c| **c != 0) // truncate string to null termination
            .map(|c| *c)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    debug!("Found self path {:?}", self_path_str);

    let addon_path = PathBuf::from(&self_path_str);
    let tmp_path = PathBuf::from(&(self_path_str.clone() + ".tmp"));
    let old_path = PathBuf::from(&(self_path_str + ".old"));

    {
        let mut file = match File::create(&tmp_path) {
            Ok(x) => x,
            Err(e) => {
                warn!("Failed to create {:?} - {:?}", tmp_path, e);
                return false;
            }
        };
        match file.write_all(&pData) {
            Ok(_) => {}
            Err(e) => {
                warn!("Failed to write to {:?} - {:?}", tmp_path, e);
                return false;
            }
        }
    }

    match fs::rename(&addon_path, &old_path) {
        Ok(_) => {}
        Err(e) => {
            warn!(
                "Failed to rename {:?} to {:?} - {:?}",
                addon_path, old_path, e
            );
            return false;
        }
    }

    match fs::rename(&tmp_path, &addon_path) {
        Ok(_) => {}
        Err(e) => {
            error!(
                "Failed to rename {:?} to {:?} - {:?}",
                addon_path, old_path, e
            );
            error!("Addon has effectively been uninstalled");
            return false;
        }
    }

    true
}

pub fn find_potential_update() {
    let current_version = Version::from(env!("CARGO_PKG_VERSION")).unwrap();

    std::thread::spawn(move || {
        if let Some(releases) = get_releases(RELEASE_REPO) {
            let release = find_potential_update_internal(&current_version, &releases, true);
            let release = match release {
                Some(x) => x,
                None => {
                    return;
                }
            };

            let download_url = match retrieve_download_url(release) {
                Some(x) => x,
                None => {
                    warn!("Failed to find download url in assets for release");
                    return;
                }
            };

            let update_info = UpdateInfo::new(release.clone(), download_url);

            *NEW_UPDATE.write() = Some(update_info);
        } else {
            warn!("Getting releases failed");
        }
    });
}

fn get_releases(pReleaseRepo: &str) -> Option<Vec<Release>> {
    let url = format!("https://api.github.com/repos/{}/releases", pReleaseRepo);
    debug!("Fetching {:?}", &url);
    let response = match ureq::get(&url).call() {
        Ok(x) => x,
        Err(e) => {
            warn!("Getting response failed {}", e);
            return None;
        }
    };
    let releases: Vec<Release> = match response.into_json() {
        Ok(x) => x,
        Err(e) => {
            warn!("Parsing response failed {}", e);
            return None;
        }
    };

    return Some(releases);
}

fn find_potential_update_internal<'a>(
    pCurrentVersion: &Version,
    pReleases: &'a Vec<Release>,
    pAllowPrelease: bool,
) -> Option<&'a Release> {
    //let current_version = Version::from(env!("CARGO_PKG_VERSION")).unwrap();

    let potential_upgrade = pReleases
        .iter()
        .find(|x| pAllowPrelease == true || x.prerelease == false);
    if let Some(potential_upgrade) = potential_upgrade {
        // The tag can take a form like "v1.3.rc4" which should get sanitized to "1.3.4"
        let sanitized_version = tag_to_version_num(&potential_upgrade.tag_name);

        if let Some(release_version) = Version::from(&sanitized_version) {
            if release_version > *pCurrentVersion {
                info!(
                    "Found update {} in tag {} (current is {})",
                    release_version, potential_upgrade.tag_name, pCurrentVersion
                );
                return Some(potential_upgrade);
            } else {
                info!(
                    "Latest applicable release {} is not an upgrade (current is {})",
                    release_version, pCurrentVersion
                );
            }
        } else {
            warn!(
                "Potential update has invalid version - {:?}",
                potential_upgrade
            );
        }
    } else {
        warn!(
            "Found no applicable releases in release repository - {:?}",
            pReleases
        );
    }

    return None;
}

fn retrieve_download_url(pRelease: &Release) -> Option<String> {
    for asset in &pRelease.assets {
        if asset.name.ends_with(".dll") {
            info!("Found download link {:?}", &asset.browser_download_url);
            return Some(asset.browser_download_url.clone());
        }
    }

    warn!("Failed to find a '.dll' asset in release - {:?}", pRelease);
    None
}

#[cfg(test)]
mod tests {
    use super::find_potential_update_internal;
    use crate::{infra::install_log_handler, updates::Release};
    use version_compare::Version;

    // Test that when self leaves squad, all squad members are dereregistered
    #[test]
    fn update() {
        install_log_handler().unwrap();

        let releases = vec![
            Release {
                prerelease: true,
                tag_name: "v2.1.rc1".to_string(),
                assets: Vec::new(),
            },
            Release {
                prerelease: false,
                tag_name: "v2.0.3".to_string(),
                assets: Vec::new(),
            },
            Release {
                prerelease: true,
                tag_name: "v2.0.rc3".to_string(),
                assets: Vec::new(),
            },
            Release {
                prerelease: true,
                tag_name: "v2.0.rc2".to_string(),
                assets: Vec::new(),
            },
            Release {
                prerelease: true,
                tag_name: "v2.0.rc1".to_string(),
                assets: Vec::new(),
            },
        ];
        // only newer pre-release
        assert_eq!(
            find_potential_update_internal(&Version::from("2.0.3").unwrap(), &releases, true),
            Some(&releases[0])
        );
        assert_eq!(
            find_potential_update_internal(&Version::from("2.0.3").unwrap(), &releases, false),
            None
        );

        // no update
        for prerelease in [true, false] {
            assert_eq!(
                find_potential_update_internal(
                    &Version::from("2.1.1").unwrap(),
                    &releases,
                    prerelease
                ),
                None
            );
        }

        // newer normal release
        let releases_2 = releases[1..].to_vec();
        for prerelease in [true, false] {
            assert_eq!(
                find_potential_update_internal(
                    &Version::from("2.0.2").unwrap(),
                    &releases_2,
                    prerelease
                ),
                Some(&releases_2[0])
            );
        }

        // Very old version
        assert_eq!(
            find_potential_update_internal(&Version::from("1.5.0").unwrap(), &releases, false),
            Some(&releases[1])
        );
    }
}
