use sqlx::migrate::Migrator;

pub struct ModuleMigrations {
    pub name: &'static str,
    pub migrator: Migrator,
}

/// Order matters once a module references another's tables, so a new module
/// goes after the ones it depends on.
pub fn all() -> Vec<ModuleMigrations> {
    vec![module("iam", sqlx::migrate!("./migrations/iam"))]
}

pub fn find(name: &str) -> Option<ModuleMigrations> {
    all().into_iter().find(|module| module.name == name)
}

pub fn names() -> Vec<&'static str> {
    all().into_iter().map(|module| module.name).collect()
}

/// Every module shares the `_sqlx_migrations` ledger, so each migrator has to
/// tolerate the versions the other modules recorded. Version numbers are
/// timestamps and therefore unique across modules, so nothing collides.
fn module(name: &'static str, mut migrator: Migrator) -> ModuleMigrations {
    migrator.set_ignore_missing(true);

    ModuleMigrations { name, migrator }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_module_is_registered_once() {
        let mut names = names();
        let count = names.len();

        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), count, "a module is registered twice");
        assert!(names.contains(&"iam"));
    }

    /// A reversible migration is two entries under one version, so only the
    /// up side is counted.
    fn up_versions(module: &ModuleMigrations) -> Vec<i64> {
        module
            .migrator
            .iter()
            .filter(|m| !m.migration_type.is_down_migration())
            .map(|m| m.version)
            .collect()
    }

    #[test]
    fn versions_are_unique_across_modules() {
        // They share one ledger, so a collision would make one module's
        // migration silently count as another's.
        let mut versions: Vec<_> = all().iter().flat_map(up_versions).collect();
        let count = versions.len();

        versions.sort_unstable();
        versions.dedup();

        assert_eq!(versions.len(), count, "two migrations share a version");
    }

    #[test]
    fn finds_a_module_by_name() {
        assert!(find("iam").is_some());
        assert!(find("nope").is_none());
    }

    #[test]
    fn the_iam_module_carries_its_migrations() {
        let iam = find("iam").expect("iam should be registered");

        assert_eq!(up_versions(&iam).len(), 11);
        assert!(iam.migrator.iter().all(|m| !m.sql.is_empty()));
    }

    #[test]
    fn every_migration_is_reversible() {
        // `just migrate-down` can only roll a release back if each up has a
        // matching down.
        for module in all() {
            let ups = up_versions(&module);

            for version in ups {
                assert!(
                    module
                        .migrator
                        .iter()
                        .any(|m| m.version == version && m.migration_type.is_down_migration()),
                    "{}/{version} has no down migration",
                    module.name
                );
            }
        }
    }
}
