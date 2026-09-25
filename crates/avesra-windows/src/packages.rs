//! Current-user package metadata only. No activation occurs in this module.
use avesra_contracts::ErrorCode;
use avesra_core::apps::{AppRecord, AppSource, LaunchIdentity};
use std::time::{Duration, Instant};
use uuid::Uuid;
use windows::{
    ApplicationModel::Package,
    Management::Deployment::PackageManager,
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::HSTRING,
};

#[derive(Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub app_id: String,
    pub package_full_name: String,
    pub publisher_id: String,
    principal: String,
}
#[derive(Default)]
pub struct Discovery {
    pub candidates: Vec<Candidate>,
    pub skipped: u32,
    pub truncated: bool,
}
struct Apartment;
impl Apartment {
    fn enter() -> Result<Self, ErrorCode> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|_| ErrorCode::Unavailable)?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() }
    }
}
fn text(value: HSTRING, max: usize) -> Result<String, ErrorCode> {
    // Bound before conversion; reject ill-formed UTF-16 rather than replacing it.
    if value.is_empty() || value.len() > max {
        return Err(ErrorCode::Malformed);
    }
    let value = String::from_utf16(&value).map_err(|_| ErrorCode::Malformed)?;
    if value.len() > max || value.chars().any(char::is_control) {
        return Err(ErrorCode::Malformed);
    }
    Ok(value)
}
fn metadata(package: &Package) -> Result<(String, String, String), ErrorCode> {
    if package.IsFramework().map_err(|_| ErrorCode::Unavailable)?
        || package
            .IsResourcePackage()
            .map_err(|_| ErrorCode::Unavailable)?
        || !package
            .Status()
            .and_then(|v| v.VerifyIsOK())
            .map_err(|_| ErrorCode::Unavailable)?
    {
        return Err(ErrorCode::Unsupported);
    }
    let id = package.Id().map_err(|_| ErrorCode::Unavailable)?;
    Ok((
        text(id.FullName().map_err(|_| ErrorCode::Unavailable)?, 512)?,
        text(id.FamilyName().map_err(|_| ErrorCode::Unavailable)?, 512)?,
        text(id.PublisherId().map_err(|_| ErrorCode::Unavailable)?, 128)?,
    ))
}
fn entries(package: &Package, principal: &str, deadline: Instant) -> Result<Discovery, ErrorCode> {
    let (full, family, publisher) = metadata(package)?;
    if Instant::now() >= deadline {
        return Ok(Discovery {
            truncated: true,
            ..Default::default()
        });
    }
    // Own the actual asynchronous operation through completion, even if the
    // admission deadline elapses. It never launches or repairs an application.
    let values = package
        .GetAppListEntriesAsync()
        .map_err(|_| ErrorCode::Unavailable)?
        .join()
        .map_err(|_| ErrorCode::Unavailable)?;
    let count = values.Size().map_err(|_| ErrorCode::Unavailable)?;
    let mut result = Discovery {
        truncated: count > 64,
        ..Default::default()
    };
    for index in 0..count.min(64) {
        if Instant::now() >= deadline {
            result.truncated = true;
            break;
        }
        let candidate = (|| {
            let entry = values.GetAt(index).map_err(|_| ErrorCode::Unavailable)?;
            let app_id = text(
                entry.AppUserModelId().map_err(|_| ErrorCode::Unavailable)?,
                512,
            )?;
            if app_id.split_once('!').map(|v| v.0) != Some(family.as_str()) {
                return Err(ErrorCode::Malformed);
            }
            let candidate = Candidate {
                name: text(
                    entry
                        .DisplayInfo()
                        .and_then(|v| v.DisplayName())
                        .map_err(|_| ErrorCode::Unavailable)?,
                    256,
                )?,
                app_id,
                package_full_name: full.clone(),
                publisher_id: publisher.clone(),
                principal: principal.into(),
            };
            candidate.record(Uuid::new_v4()).validate()?;
            Ok::<_, ErrorCode>(candidate)
        })();
        match candidate {
            Ok(candidate) => result.candidates.push(candidate),
            Err(_) => result.skipped += 1,
        }
    }
    Ok(result)
}
pub fn discover() -> Result<Discovery, ErrorCode> {
    let _apartment = Apartment::enter()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let principal = crate::principal::current_user()?;
    let manager = PackageManager::new().map_err(|_| ErrorCode::Unavailable)?;
    let packages = manager
        .FindPackagesByUserSecurityId(&HSTRING::new())
        .map_err(|_| ErrorCode::Unavailable)?;
    let iterator = packages.First().map_err(|_| ErrorCode::Unavailable)?;
    let mut result = Discovery::default();
    let mut examined = 0;
    while iterator.HasCurrent().map_err(|_| ErrorCode::Unavailable)? {
        if examined >= 512 || result.candidates.len() >= 256 || Instant::now() >= deadline {
            result.truncated = true;
            break;
        }
        examined += 1;
        match iterator
            .Current()
            .map_err(|_| ErrorCode::Unavailable)
            .and_then(|v| entries(&v, &principal, deadline))
        {
            Ok(mut batch) => {
                let remaining = 256 - result.candidates.len();
                result.truncated |= batch.truncated || batch.candidates.len() > remaining;
                batch.candidates.truncate(remaining);
                result.candidates.extend(batch.candidates);
                result.skipped = result.skipped.saturating_add(batch.skipped);
            }
            Err(_) => result.skipped += 1,
        }
        if Instant::now() >= deadline {
            result.truncated = true;
            break;
        }
        iterator.MoveNext().map_err(|_| ErrorCode::Unavailable)?;
    }
    Ok(result)
}
impl Candidate {
    fn record(&self, owner: Uuid) -> AppRecord {
        AppRecord {
            id: Uuid::new_v4(),
            revision: Uuid::new_v4(),
            selected_by: owner,
            name: self.name.clone(),
            source: AppSource::PackageRegistration,
            source_identity: Some(self.app_id.clone()),
            publisher: None,
            window_class: None,
            launch: LaunchIdentity::Packaged {
                app_id: self.app_id.clone(),
                package_full_name: self.package_full_name.clone(),
                publisher_id: self.publisher_id.clone(),
            },
        }
    }
    /// Fresh exact current-user registration; no file path or display-name match.
    pub fn revalidate(&self) -> Result<(), ErrorCode> {
        let _apartment = Apartment::enter()?;
        if crate::principal::current_user()? != self.principal {
            return Err(ErrorCode::Stale);
        }
        let manager = PackageManager::new().map_err(|_| ErrorCode::Unavailable)?;
        let package = manager
            .FindPackageByUserSecurityIdPackageFullName(
                &HSTRING::new(),
                &HSTRING::from(&self.package_full_name),
            )
            .map_err(|_| ErrorCode::Stale)?;
        let fresh = entries(
            &package,
            &self.principal,
            Instant::now() + Duration::from_secs(5),
        )?;
        if fresh.truncated
            || fresh
                .candidates
                .iter()
                .filter(|v| v.app_id == self.app_id)
                .count()
                != 1
            || !fresh.candidates.iter().any(|v| v == self)
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    }
    pub fn select(&self, owner: Uuid) -> Result<AppRecord, ErrorCode> {
        self.revalidate()?;
        let record = self.record(owner);
        record.validate()?;
        Ok(record)
    }
}
