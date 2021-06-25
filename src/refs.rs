use crate::errors::{Error, Result};
use crate::lockfile::Lockfile;
use crate::revision::Revision;
use crate::util::path_to_string;
use lazy_static::lazy_static;
use regex::Regex;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const HEAD: &str = "HEAD";

lazy_static! {
    static ref SYMREF: Regex = Regex::new(r"^ref: (.+)$").unwrap();
}

#[derive(Debug, PartialEq, Eq)]
pub enum Ref {
    SymRef { path: String },
    Ref { oid: String },
}

impl Ref {
    pub fn is_head(&self) -> bool {
        match self {
            Ref::SymRef { path } => path == HEAD,
            Ref::Ref { .. } => false,
        }
    }
}

#[derive(Debug)]
pub struct Refs {
    pathname: PathBuf,
    refs_path: PathBuf,
    heads_path: PathBuf,
}

impl Refs {
    pub fn new(pathname: PathBuf) -> Self {
        let refs_path = pathname.join("refs");
        let heads_path = refs_path.join("heads");

        Refs {
            pathname,
            refs_path,
            heads_path,
        }
    }

    pub fn update_head(&self, oid: String) -> Result<()> {
        self.update_symref(self.pathname.join(HEAD), &oid)
    }

    pub fn read_head(&self) -> Result<Option<String>> {
        self.read_symref(&self.pathname.join(HEAD))
    }

    pub fn read_ref(&self, name: &str) -> Result<Option<String>> {
        if let Some(path) = self.path_for_name(name) {
            self.read_symref(&path)
        } else {
            Ok(None)
        }
    }

    pub fn create_branch(&self, branch_name: &str, start_oid: String) -> Result<()> {
        let path = self.heads_path.join(branch_name);

        if !Revision::valid_ref(branch_name) {
            return Err(Error::InvalidBranch(format!(
                "'{}' is not a valid branch name.",
                branch_name
            )));
        }

        if path.as_path().exists() {
            return Err(Error::InvalidBranch(format!(
                "A branch named '{}' already exists.",
                branch_name
            )));
        }

        self.update_ref_file(path, &start_oid)?;

        Ok(())
    }

    pub fn set_head(&self, revision: &str, oid: &str) -> Result<()> {
        let head = self.pathname.join(HEAD);
        let path = self.heads_path.join(revision);

        if path.is_file() {
            let relative = path.strip_prefix(&self.pathname).unwrap();
            self.update_ref_file(head, &format!("ref: {}", path_to_string(relative)))?;
        } else {
            self.update_ref_file(head, &oid)?;
        }

        Ok(())
    }

    pub fn current_ref(&self, source: &str) -> Result<Ref> {
        let r#ref = self.read_oid_or_symref(&self.pathname.join(source))?;

        match r#ref {
            Some(Ref::SymRef { path }) => self.current_ref(&path),
            Some(Ref::Ref { .. }) | None => Ok(Ref::SymRef {
                path: source.to_string(),
            }),
        }
    }

    pub fn read_oid(&self, r#ref: &Ref) -> Result<Option<String>> {
        match r#ref {
            Ref::SymRef { path } => self.read_ref(&path),
            Ref::Ref { oid } => Ok(Some(oid.to_owned())),
        }
    }

    pub fn list_branches(&self) -> Result<Vec<Ref>> {
        self.list_refs(&self.heads_path)
    }

    pub fn short_name(&self, r#ref: &Ref) -> String {
        match r#ref {
            Ref::SymRef { path } => {
                let path = self.pathname.join(&path);

                let dirs = [self.heads_path.clone(), self.pathname.clone()];
                let prefix = dirs
                    .iter()
                    .find(|dir| {
                        path.parent()
                            .unwrap()
                            .ancestors()
                            .any(|parent| &parent == dir)
                    })
                    .unwrap();

                path_to_string(&path.strip_prefix(&prefix).unwrap())
            }
            Ref::Ref { .. } => unreachable!(),
        }
    }

    fn path_for_name(&self, name: &str) -> Option<PathBuf> {
        let prefixes = [
            self.pathname.clone(),
            self.refs_path.clone(),
            self.heads_path.clone(),
        ];

        for prefix in &prefixes {
            if prefix.join(name).exists() {
                return Some(prefix.join(name));
            }
        }
        None
    }

    fn update_ref_file(&self, path: PathBuf, oid: &str) -> Result<()> {
        let mut lockfile = Lockfile::new(path.clone());

        match lockfile.hold_for_update() {
            Ok(()) => (),
            Err(err) => match err {
                Error::Io(err) => {
                    if err.kind() == io::ErrorKind::NotFound {
                        // Create the parent directories and retry
                        fs::create_dir_all(path.parent().unwrap())?;
                        lockfile.hold_for_update()?;
                    } else {
                        return Err(Error::Io(err));
                    }
                }
                _ => return Err(err),
            },
        }
        self.write_lockfile(&mut lockfile, &oid)
    }

    fn read_oid_or_symref(&self, path: &Path) -> Result<Option<Ref>> {
        if path.exists() {
            let mut data = String::new();
            let mut file = File::open(&path)?;
            file.read_to_string(&mut data)?;
            let data = data.trim();

            if let Some(r#match) = SYMREF.captures(&data) {
                Ok(Some(Ref::SymRef {
                    path: r#match[1].to_string(),
                }))
            } else {
                Ok(Some(Ref::Ref {
                    oid: data.to_string(),
                }))
            }
        } else {
            Ok(None)
        }
    }

    fn read_symref(&self, path: &Path) -> Result<Option<String>> {
        let r#ref = self.read_oid_or_symref(&path)?;

        match r#ref {
            Some(Ref::SymRef { path }) => self.read_symref(&self.pathname.join(path)),
            Some(Ref::Ref { oid }) => Ok(Some(oid)),
            None => Ok(None),
        }
    }

    fn update_symref(&self, path: PathBuf, oid: &str) -> Result<()> {
        let mut lockfile = Lockfile::new(path.clone());
        lockfile.hold_for_update()?;

        let r#ref = self.read_oid_or_symref(&path)?;

        match r#ref {
            Some(Ref::Ref { .. }) | None => self.write_lockfile(&mut lockfile, &oid),
            Some(Ref::SymRef { path }) => {
                match self.update_symref(self.pathname.join(path), &oid) {
                    Ok(()) => lockfile.rollback(),
                    Err(err) => {
                        lockfile.rollback()?;
                        Err(err)
                    }
                }
            }
        }
    }

    fn write_lockfile(&self, lockfile: &mut Lockfile, oid: &str) -> Result<()> {
        lockfile.write(&format!("{}\n", oid).into_bytes())?;
        lockfile.commit()
    }

    fn list_refs(&self, dirname: &Path) -> Result<Vec<Ref>> {
        let mut result = vec![];

        for name in fs::read_dir(self.pathname.join(dirname))? {
            let path = name?.path();

            if path.is_dir() {
                result.append(&mut self.list_refs(&path)?);
            } else {
                let path = path.strip_prefix(&self.pathname).unwrap();
                result.push(Ref::SymRef {
                    path: path_to_string(path),
                });
            }
        }

        Ok(result)
    }
}
