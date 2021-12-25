#![allow(non_snake_case)]

use arcdps::{UserInfo, UserInfoIter, UserRole};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub struct SquadMemberState {
    pub join_time: u64,
    pub role: UserRole,
    pub subgroup: u8,
    pub is_ready: bool,
    pub last_ready_time: Option<Instant>,
    pub last_unready_time: Option<Instant>,
    pub total_ready_check_time: Duration,
}

impl SquadMemberState {
    fn new(join_time: u64, role: UserRole, subgroup: u8, is_ready: bool) -> Self {
        Self {
            join_time,
            role,
            subgroup,
            is_ready,
            last_ready_time: None,
            last_unready_time: None,
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

// Returns true if ready check was aborted
fn handle_ready_status_changed(
    pExistingUser: (&str, &mut SquadMemberState),
    pNow: &Instant,
) -> Option<Instant> {
    let mut ready_check_start_time: Option<Instant> = None;

    if pExistingUser.1.role == UserRole::SquadLeader {
        if pExistingUser.1.is_ready == true {
            info!("Ready check started at {:?}", pNow);
        } else {
            info!(
                "Ready check which was started at {:?} was finished at {:?}",
                pExistingUser.1.last_ready_time, pNow
            );

            ready_check_start_time = pExistingUser.1.last_ready_time;
        }
    }

    if pExistingUser.1.is_ready == true {
        pExistingUser.1.last_ready_time = Some(*pNow);
        info!("User readied up - {:?}", pExistingUser);
    } else {
        pExistingUser.1.last_unready_time = Some(*pNow);
        info!("User unreadied - {:?}", pExistingUser);
    }

    ready_check_start_time
}

// pSuccessful indicates whether the ready check was finished because everyone readied up (true) or because it was
// aborted (false)
fn handle_ready_check_finished(
    pSquadMembers: &mut HashMap<String, SquadMemberState>,
    pReadyCheckStartTime: &Instant,
    pNow: &Instant,
) {
    let mut users: Vec<(&String, &mut SquadMemberState, Duration)> = Vec::new();

    let squad_member_count = pSquadMembers.len();
    for (account_name, state) in pSquadMembers.iter_mut() {
        if let Some(ready_time) = state.last_ready_time {
            if ready_time < *pReadyCheckStartTime {
                info!(
                    "User readied before ready check started - {:?} {:?} {:?}",
                    pReadyCheckStartTime, account_name, state
                );
                continue;
            }

            if state.last_unready_time > Some(ready_time)
                && state.last_unready_time < Some(*pNow - Duration::from_millis(500))
            {
                info!(
                    "User unreadied during ready check - {:?} {:?} {:?}",
                    pReadyCheckStartTime, account_name, state
                );
                continue;
            }

            let time_spent_unready = ready_time - *pReadyCheckStartTime;
            users.push((account_name, state, time_spent_unready));
        }
    }

    // if successful
    if users.len() == squad_member_count {
        info!(
            "Ready check was successful ({} players readied)",
            users.len()
        );
        for (account_name, state, time_spent_unready) in users {
            debug!(
                "{:?} spent {:?} in ready check",
                account_name, time_spent_unready
            );
            state.total_ready_check_time += time_spent_unready;
        }
    } else {
        info!("Ready check was aborted ({} players readied)", users.len());
    }
}

pub struct SquadTracker {
    self_account_name: String,
    squad_members: HashMap<String, SquadMemberState>,
}

impl SquadTracker {
    pub fn new(self_account_name: &str) -> Self {
        Self {
            self_account_name: String::from(self_account_name),
            squad_members: HashMap::new(),
        }
    }

    pub fn squad_update(&mut self, pUsers: UserInfoIter) {
        let now = Instant::now();

        let SquadTracker {
            self_account_name,
            squad_members,
        } = &mut *self;

        info!("Receiving {:?} updates", pUsers.len());
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
                    let entry = squad_members.entry(account_name.to_string());
                    let new_user_state = match entry {
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

                    let ready_check_started_time = if let Some(new_user_state) = new_user_state {
                        handle_ready_status_changed((account_name, new_user_state), &now)
                    } else {
                        None
                    };

                    if let Some(start_time) = ready_check_started_time {
                        handle_ready_check_finished(squad_members, &start_time, &now);
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

    pub fn get_squad_members(&self) -> &HashMap<String, SquadMemberState> {
        &self.squad_members
    }

    pub fn setup_mock_data(&mut self) {
        let now = Instant::now();

        assert_eq!(self.squad_members, HashMap::new());
        self.squad_members.insert(
            "Alice".to_string(),
            SquadMemberState::new(100, UserRole::Member, 0, false),
        );
        self.squad_members
            .get_mut("Alice")
            .unwrap()
            .total_ready_check_time = Duration::new(100, 0);
        self.squad_members.insert(
            "Bob".to_string(),
            SquadMemberState::new(100, UserRole::SquadLeader, 0, true),
        );
        self.squad_members.get_mut("Bob").unwrap().last_ready_time =
            Some(now - Duration::new(0, 0));
        self.squad_members
            .get_mut("Bob")
            .unwrap()
            .total_ready_check_time = Duration::new(200, 0);
        self.squad_members.insert(
            "Charlie".to_string(),
            SquadMemberState::new(100, UserRole::Member, 0, true),
        );
        self.squad_members
            .get_mut("Charlie")
            .unwrap()
            .last_ready_time = Some(now - Duration::new(10, 0));
        self.squad_members
            .get_mut("Charlie")
            .unwrap()
            .total_ready_check_time = Duration::new(100, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::SquadTracker;
    use crate::infra::install_log_handler;
    use arcdps::{RawUserInfo, UserInfoIter, UserRole};
    use more_asserts::*;
    use rstest::rstest;
    use std::mem::MaybeUninit;
    use std::time::{Duration, Instant};

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

    fn ready_player(pPlayerName: &str, pTracker: &mut SquadTracker, pTestUsers: &mut TestUserList) {
        pTestUsers.users.clear();
        pTestUsers.users.push(TestUser::new(
            pPlayerName.to_string(),
            pTracker.squad_members[pPlayerName].join_time,
            pTracker.squad_members[pPlayerName].role,
            pTracker.squad_members[pPlayerName].subgroup,
            true,
        ));
        let mut expected_state = pTracker.squad_members.clone();
        let pre_op_time = Instant::now();
        unsafe {
            pTracker.squad_update(pTestUsers.get_iter());
        }
        let post_op_time = Instant::now();

        if expected_state[pPlayerName].is_ready == false {
            assert_in_range!(
                pTracker.squad_members[pPlayerName].last_ready_time,
                Some(pre_op_time),
                Some(post_op_time)
            );

            expected_state.get_mut(pPlayerName).unwrap().is_ready = true;
            expected_state.get_mut(pPlayerName).unwrap().last_ready_time =
                pTracker.squad_members[pPlayerName].last_ready_time;
        }

        assert_eq!(pTracker.squad_members, expected_state);
    }

    fn unready_player(
        pPlayerName: &str,
        pTracker: &mut SquadTracker,
        pTestUsers: &mut TestUserList,
    ) {
        pTestUsers.users.clear();
        pTestUsers.users.push(TestUser::new(
            pPlayerName.to_string(),
            12345,
            UserRole::Member,
            0,
            false,
        ));
        let mut expected_state = pTracker.squad_members.clone();
        let pre_op_time = Instant::now();
        unsafe {
            pTracker.squad_update(pTestUsers.get_iter());
        }
        let post_op_time = Instant::now();

        if expected_state[pPlayerName].is_ready == true {
            assert_in_range!(
                pTracker.squad_members[pPlayerName].last_unready_time,
                Some(pre_op_time),
                Some(post_op_time)
            );

            expected_state.get_mut(pPlayerName).unwrap().is_ready = false;
            expected_state
                .get_mut(pPlayerName)
                .unwrap()
                .last_unready_time = pTracker.squad_members[pPlayerName].last_unready_time;
        }

        assert_eq!(pTracker.squad_members, expected_state);
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
    fn ready_check(
        #[values(false, true)] pAborted: bool,
        #[values(false, true)] pReadyAndUnready: bool,
    ) {
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
        test_users.users.push(TestUser::new(
            "peer".to_string(),
            12345,
            UserRole::Member,
            0,
            false,
        ));
        let pre_op_time = Instant::now();
        unsafe {
            tracker.squad_update(test_users.get_iter());
        }
        let post_op_time = Instant::now();
        assert_eq!(tracker.squad_members.len(), 3);

        let initial_ready_check_time_spent = Duration::new(5, 0);
        for user in ["self", "peer", "squad_leader"] {
            tracker
                .squad_members
                .get_mut(user)
                .unwrap()
                .total_ready_check_time = initial_ready_check_time_spent;

            assert_eq!(tracker.squad_members[user].is_ready, false);
            assert_le!(
                tracker.squad_members[user].last_ready_time,
                Some(pre_op_time)
            );
            assert_le!(
                tracker.squad_members[user].last_unready_time,
                Some(post_op_time)
            );
        }

        ready_player("squad_leader", &mut tracker, &mut test_users);
        ready_player("peer", &mut tracker, &mut test_users);
        if pReadyAndUnready == true {
            unready_player("peer", &mut tracker, &mut test_users);
            ready_player("peer", &mut tracker, &mut test_users);
        }

        if pAborted == false {
            ready_player("self", &mut tracker, &mut test_users);
        }

        // Ready check finished. Players unready in "random" order. If aborted, the state should not have changed from
        // before the ready check was started
        let mut expected_state = tracker.squad_members.clone();
        for user in ["self", "squad_leader", "peer"] {
            test_users.users.clear();
            let role = if user == "squad_leader" {
                UserRole::SquadLeader
            } else {
                UserRole::Member
            };
            test_users
                .users
                .push(TestUser::new(user.to_string(), 12345, role, 0, false));
            let pre_op_time = Instant::now();
            unsafe {
                tracker.squad_update(test_users.get_iter());
            }
            let post_op_time = Instant::now();

            if expected_state[user].is_ready == true {
                assert_in_range!(
                    tracker.squad_members[user].last_unready_time,
                    Some(pre_op_time),
                    Some(post_op_time)
                );
                expected_state.get_mut(user).unwrap().last_unready_time =
                    tracker.squad_members[user].last_unready_time;
                expected_state.get_mut(user).unwrap().is_ready = false;
            }
        }

        if pAborted == false {
            for user in ["self", "peer"] {
                assert_gt!(
                    tracker.squad_members[user].total_ready_check_time,
                    initial_ready_check_time_spent
                );

                expected_state.get_mut(user).unwrap().total_ready_check_time =
                    tracker.squad_members[user].total_ready_check_time;
            }
        }

        assert_eq!(tracker.squad_members, expected_state);
    }

    #[rstest]
    fn ready_check_during_restart(
        #[values(false, true)] pAborted: bool,
        #[values(false, true)] pReadyAndUnready: bool,
    ) {
        install_log_handler().unwrap();

        let mut tracker = SquadTracker::new("self");
        let mut test_users = TestUserList::new();

        // Squad setup - two players are already ready of which one is the squad leader
        test_users.users.push(TestUser::new(
            "squad_leader".to_string(),
            12345,
            UserRole::SquadLeader,
            0,
            true,
        ));
        test_users.users.push(TestUser::new(
            "self".to_string(),
            12345,
            UserRole::Member,
            0,
            false,
        ));
        test_users.users.push(TestUser::new(
            "peer".to_string(),
            12345,
            UserRole::Member,
            0,
            true,
        ));
        unsafe {
            tracker.squad_update(test_users.get_iter());
        }

        assert_eq!(tracker.squad_members.len(), 3);
        let initial_ready_check_time_spent = Duration::new(5, 0);
        for user in ["self", "peer", "squad_leader"] {
            tracker
                .squad_members
                .get_mut(user)
                .unwrap()
                .total_ready_check_time = initial_ready_check_time_spent;
        }

        if pReadyAndUnready == true {
            unready_player("peer", &mut tracker, &mut test_users);
            ready_player("peer", &mut tracker, &mut test_users);
        }

        if pAborted == false {
            ready_player("self", &mut tracker, &mut test_users);
        }

        // Ready check finished. Players unready in "random" order. If aborted, the state should not have changed from
        // before the ready check was started
        let mut expected_state = tracker.squad_members.clone();
        for user in ["self", "squad_leader", "peer"] {
            test_users.users.clear();
            let role = if user == "squad_leader" {
                UserRole::SquadLeader
            } else {
                UserRole::Member
            };
            test_users
                .users
                .push(TestUser::new(user.to_string(), 12345, role, 0, false));
            let pre_op_time = Instant::now();
            unsafe {
                tracker.squad_update(test_users.get_iter());
            }
            let post_op_time = Instant::now();

            if expected_state[user].is_ready == true {
                assert_in_range!(
                    tracker.squad_members[user].last_unready_time,
                    Some(pre_op_time),
                    Some(post_op_time)
                );
                expected_state.get_mut(user).unwrap().last_unready_time =
                    tracker.squad_members[user].last_unready_time;
                expected_state.get_mut(user).unwrap().is_ready = false;
            }
        }

        if pAborted == false {
            assert_gt!(
                tracker.squad_members["self"].total_ready_check_time,
                initial_ready_check_time_spent
            );

            expected_state
                .get_mut("self")
                .unwrap()
                .total_ready_check_time = tracker.squad_members["self"].total_ready_check_time;

            // Peer readied at the first possible moment, so the increment could be zero unless they did a ready-unready cycle
            if pReadyAndUnready == true {
                assert_gt!(
                    tracker.squad_members["peer"].total_ready_check_time,
                    initial_ready_check_time_spent
                );

                expected_state
                    .get_mut("peer")
                    .unwrap()
                    .total_ready_check_time = tracker.squad_members["peer"].total_ready_check_time;
            } else {
                assert_eq!(
                    tracker.squad_members["peer"].total_ready_check_time,
                    initial_ready_check_time_spent
                );
            }
        }

        assert_eq!(tracker.squad_members, expected_state);
    }
}
