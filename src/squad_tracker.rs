#![allow(non_snake_case)]

use arcdps::{UserInfo, UserInfoIter, UserRole};
use std::collections::HashMap;
use std::time::{Duration, Instant};

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
        for user in pUsers.into_iter() {
            info!("{:?}", user);

            let account_name = match user.account_name {
                Some(x) => x,
                None => continue,
            };

            match user.role {
                UserRole::SquadLeader | UserRole::Lieutenant | UserRole::Member => {
                    // TODO: ready status here? How would it be handled if ready_status was true?
                    let mut result = self.squad_members.insert(
                        account_name.to_string(),
                        SquadMemberState::new(
                            user.join_time,
                            user.role,
                            user.subgroup,
                            user.ready_status,
                        ),
                    );
                    match &mut result {
                        Some(existing_user) => {
                            self.handle_user_update(existing_user, &user, &now);
                        }
                        None => info!("Added new player ({}) to the squad", account_name),
                    };
                }
                UserRole::None => {
                    if account_name == self.self_account_name {
                        info!("Self ({}) left - clearing squad", account_name);
                        self.squad_members.clear();
                    } else {
                        let result = self.squad_members.remove(account_name);
                        if result.is_some() {
                            info!("Removed {} from the squad", account_name);
                        } else {
                            info!("Couldn't find {}, who left, in the squad map, they were probably invited and the invite was cancelled", account_name);
                        }
                    }
                }
                _ => {} // Ignore entry
            };
        }
    }

    fn handle_user_update(
        &mut self,
        pExistingUser: &mut SquadMemberState,
        pUpdate: &UserInfo,
        pNow: &Instant,
    ) {
        if pExistingUser.is_ready != pUpdate.ready_status {
            match pUpdate.role {
                UserRole::SquadLeader => match pUpdate.ready_status {
                    true => {
                        self.ready_check_started_time = Some(*pNow);
                        info!("Ready check started at {:?}", pNow);
                    }
                    false => {
                        self.ready_check_started_time = None;
                        info!(
                            "Ready check which was started at {:?} was aborted at {:?}",
                            self.ready_check_started_time, pNow
                        );
                    }
                },
                _ => match pUpdate.ready_status {
                    true => {
                        match self.ready_check_started_time {
                            Some(start_time) => {
                                let time_spent = *pNow - start_time;
                                pExistingUser.ready_check_time_spent += time_spent;
                                info!(
                                    "User {:?} readied up - they spent {:?} doing so",
                                    pUpdate, time_spent
                                )
                            }
                            None => {
                                info!(
                                    "User {:?} readied up when there was no ready check active",
                                    pUpdate
                                )
                            }
                        };
                    }
                    false => {
                        self.ready_check_started_time = None;
                    }
                },
            }
        }

        pExistingUser.update_user(pUpdate);
    }
}
