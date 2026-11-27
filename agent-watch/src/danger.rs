//! Alerts: a credential file opened by a program that is not its own tool.

use crate::constants::{
    CREDENTIALS, FIRST_USER_UID, PASSWD_GID_FIELD, PASSWD_HOME_FIELD, PASSWD_SEP, PASSWD_UID_FIELD,
    PATH_SEP, ROOT_UID,
};

/// Each credential path under each checked home, with the programs that own it.
pub struct Credentials(Vec<(String, &'static [&'static str])>);

impl Credentials {
    pub fn new(homes: &[String]) -> Self {
        let home_paths = homes.iter().map(|home| home.trim_end_matches(PATH_SEP));
        let paths = home_paths
            .flat_map(|home| {
                CREDENTIALS
                    .iter()
                    .map(move |&(folder, owners)| (format!("{home}{PATH_SEP}{folder}"), owners))
            })
            .collect();
        Credentials(paths)
    }

    /// True when comm opened a credential path, or one inside it, that is not its own.
    pub fn foreign(&self, comm: &str, path: &str) -> bool {
        self.0.iter().any(|(credential, owners)| {
            let inside = path
                .strip_prefix(credential.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(PATH_SEP));
            inside && !owners.contains(&comm)
        })
    }
}

/// A login account: root, or uid 1000 and up.
pub struct Account {
    pub uid: u32,
    pub gid: u32,
    pub home: String,
}

/// The login accounts in the text of a passwd file.
pub fn accounts(passwd: &str) -> Vec<Account> {
    passwd
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(PASSWD_SEP).collect();
            let number = |field: usize| fields.get(field)?.parse::<u32>().ok();
            let (uid, gid) = (number(PASSWD_UID_FIELD)?, number(PASSWD_GID_FIELD)?);
            let home = fields.get(PASSWD_HOME_FIELD)?.to_string();
            (uid == ROOT_UID || uid >= FIRST_USER_UID).then_some(Account { uid, gid, home })
        })
        .collect()
}

/// The homes of these accounts.
pub fn homes(accounts: &[Account]) -> Vec<String> {
    accounts
        .iter()
        .map(|account| account.home.clone())
        .collect()
}

#[cfg(test)]
#[path = "danger_tests.rs"]
mod tests;
