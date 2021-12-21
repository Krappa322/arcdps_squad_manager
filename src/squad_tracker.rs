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
    current_ready_check_time: Option<Duration>,
    total_ready_check_time: Duration,
}

impl SquadMemberState {
    fn new(join_time: u64, role: UserRole, subgroup: u8, is_ready: bool) -> Self {
        Self {
            join_time,
            role,
            subgroup,
            is_ready,
            current_ready_check_time: None,
            total_ready_check_time: Duration::new(0, 0),
        }
    }

    fn update_user(&mut self, pUpdate: &UserInfo) {
        self.join_time = pUpdate.join_time; // join_time could update if we somehow missed an update and the user left then rejoined
        self.role = pUpdate.role;
        self.subgroup = pUpdate.subgroup;
        self.is_ready = pUpdate.ready_status;
    }
}

// Returns the time spent in ready check if a ready check was completed
fn handle_ready_status_changed(
    pReadyCheckStartedTime: &mut Option<Instant>,
    pExistingUser: &mut SquadMemberState,
    pNow: &Instant,
) -> Option<Duration> {
    let mut ready_check_duration: Option<Duration> = None;

    match pExistingUser.role {
        UserRole::SquadLeader => {
            if pExistingUser.is_ready == true {
                *pReadyCheckStartedTime = Some(*pNow);
                info!("Ready check started at {:?}", pNow);
            } else {
                info!(
                    "Ready check which was started at {:?} was aborted at {:?}",
                    pReadyCheckStartedTime, pNow
                );
                if let Some(start_time) = pReadyCheckStartedTime {
                    ready_check_duration = Some(*pNow - *start_time);
                }
                *pReadyCheckStartedTime = None;
            }
        }
        _ => {}
    }

    if pExistingUser.is_ready == true {
        if let Some(start_time) = pReadyCheckStartedTime {
            pExistingUser.current_ready_check_time = Some(*pNow - *start_time);
            info!(
                "User {:?} readied up - they spent {:?} doing so",
                pExistingUser, pExistingUser.current_ready_check_time
            )
        } else {
            info!(
                "User {:?} readied up when there was no ready check active",
                pExistingUser
            )
        }
    } else if ready_check_duration.is_none() { // User can't unready if ready check is finished
        if let Some(time_spent) = pExistingUser.current_ready_check_time {
            info!(
                "User {:?} unreadied - current_ready_check_time={:?}",
                pExistingUser, time_spent
            )
        }
        pExistingUser.current_ready_check_time = None;
    }

    ready_check_duration
}

fn handle_ready_check_completed(
    pSquadMembers: &mut HashMap<String, SquadMemberState>,
    pReadyCheckDuration: Duration,
) {
    info!(
        "handle_ready_check_completed {:?} {:?}",
        &pSquadMembers,
        pReadyCheckDuration
    );

    for (_account_name, state) in pSquadMembers {
        let time_spent = state
            .current_ready_check_time
            .unwrap_or(pReadyCheckDuration);

        state.total_ready_check_time += time_spent;
        state.current_ready_check_time = None;
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

                    let mut ready_check_duration: Option<Duration> = None;
                    if let Some(new_user_state) = new_user_state {
                        ready_check_duration = handle_ready_status_changed(
                            ready_check_started_time,
                            new_user_state,
                            &now,
                        );
                    }

                    if let Some(ready_check_duration) = ready_check_duration {
                        handle_ready_check_completed(squad_members, ready_check_duration);
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
    use crate::infra::install_log_handler;
    use arcdps::{RawUserInfo, UserInfoIter, UserRole};
    use more_asserts::assert_gt;
    use rstest::rstest;
    use std::mem::MaybeUninit;
    use std::time::Duration;

    struct TestUser {
        account_name: String,
        join_time: u64,
        role: UserRole,
        subgroup: u8,
        ready_status: bool,
    }

    impl TestUser {
        fn new(
            mut account_name: String,
            join_time: u64,
            role: UserRole,
            subgroup: u8,
            ready_status: bool,
        ) -> Self {
            account_name.push('\0');
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

    // Test that when self leaves squad, all squad members are dereregistered
    #[test]
    fn deregister_self() {
        install_log_handler().unwrap();

        let mut tracker = SquadTracker::new("self");
        let mut test_users = TestUserList::new();
        test_users.users.push(TestUser::new(
            "self".to_string(),
            12345,
            UserRole::Member,
            0,
            false,
        ));
        test_users.users.push(TestUser::new(
            "squad_leader".to_string(),
            12345,
            UserRole::SquadLeader,
            0,
            false,
        ));

        unsafe {
            tracker.squad_update(test_users.get_iter());
        }
        assert_eq!(tracker.squad_members.len(), 2);
        assert!(tracker.squad_members.contains_key(&"self".to_string()));
        assert!(tracker
            .squad_members
            .contains_key(&"squad_leader".to_string()));

        test_users.users.clear();
        test_users.users.push(TestUser::new(
            "self".to_string(),
            12345,
            UserRole::None,
            0,
            false,
        ));
        unsafe {
            tracker.squad_update(test_users.get_iter());
        }
        assert_eq!(tracker.squad_members.len(), 0);
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn ready_check(#[case] pAborted: bool) {
        install_log_handler().unwrap();

        let mut tracker = SquadTracker::new("self");
        let mut test_users = TestUserList::new();

        // Squad setup
        test_users.users.push(TestUser::new(
            "squad_leader".to_string(),
            12345,
            UserRole::SquadLeader,
            0,
            false,
        ));
        test_users.users.push(TestUser::new(
            "self".to_string(),
            12345,
            UserRole::Member,
            0,
            false,
        ));

        unsafe {
            tracker.squad_update(test_users.get_iter());
        }
        assert!(tracker.ready_check_started_time.is_none());
        assert_eq!(tracker.squad_members.len(), 2);
        assert_eq!(tracker.squad_members["squad_leader"].is_ready, false);
        assert_eq!(tracker.squad_members["self"].is_ready, false);

        // Ready check started
        test_users.users.clear();
        test_users.users.push(TestUser::new(
            "squad_leader".to_string(),
            12345,
            UserRole::SquadLeader,
            0,
            true,
        ));
        unsafe {
            tracker.squad_update(test_users.get_iter());
        }
        assert!(tracker.ready_check_started_time.is_some());
        assert_eq!(tracker.squad_members.len(), 2);
        assert_eq!(tracker.squad_members["squad_leader"].is_ready, true);
        assert_eq!(
            tracker.squad_members["squad_leader"].current_ready_check_time,
            Some(Duration::new(0, 0))
        );
        assert_eq!(
            tracker.squad_members["squad_leader"].total_ready_check_time,
            Duration::new(0, 0)
        );
        assert_eq!(tracker.squad_members["self"].is_ready, false);
        assert!(tracker.squad_members["self"]
            .current_ready_check_time
            .is_none());
        assert_eq!(
            tracker.squad_members["self"].total_ready_check_time,
            Duration::new(0, 0)
        );

        if pAborted == false {
            // Self readies up
            test_users.users.clear();
            test_users.users.push(TestUser::new(
                "self".to_string(),
                12345,
                UserRole::Member,
                0,
                true,
            ));
            unsafe {
                tracker.squad_update(test_users.get_iter());
            }
            assert!(tracker.ready_check_started_time.is_some());
            assert_eq!(tracker.squad_members.len(), 2);
            assert_eq!(tracker.squad_members["squad_leader"].is_ready, true);
            assert_eq!(
                tracker.squad_members["squad_leader"].current_ready_check_time,
                Some(Duration::new(0, 0))
            );
            assert_eq!(
                tracker.squad_members["squad_leader"].total_ready_check_time,
                Duration::new(0, 0)
            );
            assert_eq!(tracker.squad_members["self"].is_ready, true);
            assert_gt!(
                tracker.squad_members["self"].current_ready_check_time,
                Some(Duration::new(0, 0))
            );
            assert_eq!(
                tracker.squad_members["self"].total_ready_check_time,
                Duration::new(0, 0)
            );
        }

        // Ready check finished. Ready check time spent should be incremented regardless of whether the ready check
        // finished successfully or not
        test_users.users.clear();
        test_users.users.push(TestUser::new(
            "squad_leader".to_string(),
            12345,
            UserRole::SquadLeader,
            0,
            false,
        ));
        test_users.users.push(TestUser::new(
            "self".to_string(),
            12345,
            UserRole::Member,
            0,
            false,
        ));
        unsafe {
            tracker.squad_update(test_users.get_iter());
        }
        assert!(tracker.ready_check_started_time.is_none());
        assert_eq!(tracker.squad_members.len(), 2);
        assert_eq!(tracker.squad_members["squad_leader"].is_ready, false);
        assert!(tracker.squad_members["squad_leader"]
            .current_ready_check_time
            .is_none());
        assert_eq!(
            tracker.squad_members["squad_leader"].total_ready_check_time,
            Duration::new(0, 0)
        );
        assert_eq!(tracker.squad_members["self"].is_ready, false);
        assert!(tracker.squad_members["self"]
            .current_ready_check_time
            .is_none());
        assert_gt!(
            tracker.squad_members["self"].total_ready_check_time,
            Duration::new(0, 0)
        );

        // Perform another ready check and assert that time spent increments
        let old_ready_check_time_spent = tracker.squad_members["self"].total_ready_check_time;
        test_users.users.clear();
        test_users.users.push(TestUser::new(
            "squad_leader".to_string(),
            12345,
            UserRole::SquadLeader,
            0,
            true,
        ));
        unsafe {
            tracker.squad_update(test_users.get_iter());
        }
        assert!(tracker.ready_check_started_time.is_some());

        test_users.users.clear();
        test_users.users.push(TestUser::new(
            "self".to_string(),
            12345,
            UserRole::Member,
            0,
            true,
        ));
        test_users.users.push(TestUser::new(
            "squad_leader".to_string(),
            12345,
            UserRole::SquadLeader,
            0,
            false,
        ));
        test_users.users.push(TestUser::new(
            "self".to_string(),
            12345,
            UserRole::Member,
            0,
            false,
        ));
        unsafe {
            tracker.squad_update(test_users.get_iter());
        }
        assert!(tracker.ready_check_started_time.is_none());
        assert_eq!(tracker.squad_members.len(), 2);
        assert_eq!(tracker.squad_members["squad_leader"].is_ready, false);
        assert_eq!(
            tracker.squad_members["squad_leader"].total_ready_check_time,
            Duration::new(0, 0)
        );
        assert_eq!(tracker.squad_members["self"].is_ready, false);
        assert_gt!(
            tracker.squad_members["self"].total_ready_check_time,
            old_ready_check_time_spent
        );
    }
}
