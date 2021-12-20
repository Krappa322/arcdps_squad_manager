#![allow(non_snake_case)]

use arcdps::{UserInfo, UserInfoIter, UserRole};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct SquadMemberState {
    join_time: u64,
    role: UserRole,
    subgroup: u8,
    is_ready: bool,
    ready_check_time_spent: Duration,
}

impl SquadMemberState {
    fn new(join_time: u64, role: UserRole, subgroup: u8, is_ready: bool) -> Self {
        Self {
            join_time,
            role,
            subgroup,
            is_ready,
            ready_check_time_spent: Duration::new(0, 0),
        }
    }

    fn update_user(&mut self, pUpdate: &UserInfo) {
        self.join_time = pUpdate.join_time; // join_time could update if we somehow missed an update and the user left then rejoined
        self.role = pUpdate.role;
        self.subgroup = pUpdate.subgroup;
        self.is_ready = pUpdate.ready_status;
    }
}

fn handle_ready_status_changed(
    pReadyCheckStartedTime: &mut Option<Instant>,
    pExistingUser: &mut SquadMemberState,
    pNow: &Instant,
) {
    match pExistingUser.role {
        UserRole::SquadLeader => {
            if pExistingUser.is_ready == true {
                *pReadyCheckStartedTime = Some(*pNow);
                info!("Ready check started at {:?}", pNow);
            } else {
                *pReadyCheckStartedTime = None;
                info!(
                    "Ready check which was started at {:?} was aborted at {:?}",
                    pReadyCheckStartedTime, pNow
                );
            }
        }
        _ => {
            if pExistingUser.is_ready == true {
                match pReadyCheckStartedTime {
                    Some(start_time) => {
                        let time_spent = *pNow - *start_time;
                        pExistingUser.ready_check_time_spent += time_spent;
                        info!(
                            "User {:?} readied up - they spent {:?} doing so",
                            pExistingUser, time_spent
                        )
                    }
                    None => {
                        info!(
                            "User {:?} readied up when there was no ready check active",
                            pExistingUser
                        )
                    }
                };
            } else {
                *pReadyCheckStartedTime = None;
            }
        }
    };
}

pub struct SquadTracker {
    self_account_name: String,
    squad_members: HashMap<String, SquadMemberState>,
    ready_check_started_time: Option<Instant>,
}

impl SquadTracker {
    pub fn new(self_account_name: &str) -> Self {
        Self {
            self_account_name: String::from(self_account_name),
            squad_members: HashMap::new(),
            ready_check_started_time: None,
        }
    }

    pub fn squad_update(&mut self, pUsers: UserInfoIter) {
        let now = Instant::now();

        let SquadTracker {
            self_account_name,
            squad_members,
            ready_check_started_time,
        } = &mut *self;

        for user_update in pUsers.into_iter() {
            info!("{:?}", user_update);

            let account_name = match user_update.account_name {
                Some(x) => x,
                None => continue,
            };

            match user_update.role {
                UserRole::SquadLeader | UserRole::Lieutenant | UserRole::Member => {
                    // Either insert a new entry or update the existing one. Returns a reference to the user state if
                    // the ready check status updated (meaning further handling needs to be done to update fields)
                    let new_user_state = match squad_members.entry(account_name.to_string()) {
                        Entry::Occupied(entry) => {
                            let user = entry.into_mut();
                            let old_ready_status = user.is_ready;
                            user.update_user(&user_update);

                            if old_ready_status != user.is_ready {
                                Some(user)
                            } else {
                                None
                            }
                        }
                        Entry::Vacant(entry) => {
                            info!("Adding new player ({:?}) to the squad", user_update);
                            let user = entry.insert(SquadMemberState::new(
                                user_update.join_time,
                                user_update.role,
                                user_update.subgroup,
                                user_update.ready_status,
                            ));

                            if user_update.ready_status == true {
                                Some(user)
                            } else {
                                None
                            }
                        }
                    };

                    if let Some(new_user_state) = new_user_state {
                        handle_ready_status_changed(ready_check_started_time, new_user_state, &now);
                    }
                }
                UserRole::None => {
                    if account_name == self_account_name {
                        info!("Self ({}) left - clearing squad", account_name);
                        squad_members.clear();
                    } else {
                        let result = squad_members.remove(account_name);
                        if result.is_some() {
                            info!("Removed {} from the squad", account_name);
                        } else {
                            info!("Couldn't find {}, who left, in the squad map, they were probably invited and the invite was cancelled", account_name);
                        }
                    }
                }
                UserRole::Invited | UserRole::Applied | UserRole::Invalid => {}
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SquadTracker;
    use arcdps::{RawUserInfo, UserInfoIter, UserRole};
    use std::mem::MaybeUninit;

    struct TestUser {
        account_name: String,
        join_time: u64,
        role: UserRole,
        subgroup: u8,
        ready_status: bool,
    }

    impl TestUser {
        fn new(
            account_name: String,
            join_time: u64,
            role: UserRole,
            subgroup: u8,
            ready_status: bool,
        ) -> Self {
            Self {
                account_name,
                join_time,
                role,
                subgroup,
                ready_status,
            }
        }

        unsafe fn to_raw_user(&self) -> RawUserInfo {
            let mut result = MaybeUninit::<RawUserInfo>::zeroed().assume_init();
            result.account_name = self.account_name.as_ptr();
            result.join_time = self.join_time;
            result.role = self.role;
            result.subgroup = self.subgroup;
            result.ready_status = self.ready_status;

            result
        }
    }

    struct TestUserList {
        users: Vec<TestUser>,
        current_iterator: Vec<RawUserInfo>,
    }

    impl TestUserList {
        fn new() -> Self {
            Self {
                users: Vec::new(),
                current_iterator: Vec::new(),
            }
        }

        unsafe fn get_iter(&mut self) -> UserInfoIter {
            self.current_iterator = self
                .users
                .iter()
                .map(|x| x.to_raw_user())
                .collect::<Vec<RawUserInfo>>();

            self.current_iterator
                .iter()
                .map(::arcdps::helpers::convert_extras_user as ::arcdps::UserConvert)
        }
    }

    #[test]
    fn deregister_self() {
        let mut tracker = SquadTracker::new("self");
        let mut test_users = TestUserList::new();
        test_users.users.push(TestUser::new(
            "self".to_string(),
            12345,
            UserRole::SquadLeader,
            0,
            false,
        ));

        unsafe {
            tracker.squad_update(test_users.get_iter());
            assert_eq!(tracker.squad_members.len(), 1);

            let x = tracker.squad_members.entry("self".to_string());
        }
    }
}
