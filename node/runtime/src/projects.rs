use node_primitives::Hash;
use parity_codec::{Decode, Encode};
use rstd::prelude::*;
use support::{decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageMap};
use system::{self, ensure_signed};

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type ProjectHash = Hash; // Reference supplied externally
pub type ProjectStatus = u16; // Reference supplied externally

#[derive(PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct DeletedProject<AccountId, ProjectStatus> {
    pub owned_by: AccountId,
    pub deleted_by: AccountId,
    pub status: ProjectStatus,
}

decl_storage! {
    trait Store for Module<T: Trait> as ProjectModule {
        ProjectHashStatus get(project_hash_status): map ProjectHash => Option<ProjectStatus>;
        DeletedProjects get(deleted_project): map ProjectHash => Vec<DeletedProject<T::AccountId, ProjectStatus>>;
        ProjectHashOwner get(project_hash_owner): map ProjectHash => Option<T::AccountId>;
        OwnerProjectsList get(owner_projects_list): map T::AccountId => Vec<ProjectHash>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;

        fn add_new_project(origin, project_hash: ProjectHash) -> Result {

            // Check that the project does not exist
            ensure!(!<ProjectHashStatus<T>>::exists(&project_hash), "The project already exists!");

            // Check that the project was not deleted already
            ensure!(!<DeletedProjects<T>>::exists(&project_hash), "The project was already deleted!");

            // proceed to store project
            let who = ensure_signed(origin)?;
            let project_status: ProjectStatus = 0;

            // TODO limit nr of Projects per Account.
            <ProjectHashStatus<T>>::insert(&project_hash, &project_status);
            <ProjectHashOwner<T>>::insert(&project_hash, &who);
            <OwnerProjectsList<T>>::mutate(&who, |owner_projects_list| owner_projects_list.push(project_hash.clone()));

            Self::deposit_event(RawEvent::ProjectRegistered(project_hash, who));

            Ok(())
        }

        fn remove_project(origin, project_hash: ProjectHash) -> Result {
            ensure!(<ProjectHashStatus<T>>::exists(&project_hash), "The project does not exist!");

            // get project by hash
            let project_owner: T::AccountId = Self::project_hash_owner(&project_hash).ok_or("Error fetching project owner")?;

            // check transaction is signed.
            let changer: T::AccountId = ensure_signed(origin)?;

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise onlu the owner can change the data
            ensure!(project_owner == changer, "You cannot delete a project you do not own");

            let mut changed_by: T::AccountId = changer.clone();
            let project_status: ProjectStatus = 99;
            let deleted_project_struct = DeletedProject {
                owned_by: project_owner.clone(),
                deleted_by: changed_by.clone(),
                status: project_status
            };

            // retain all other projects except the one we want to delete
            <OwnerProjectsList<T>>::mutate(&project_owner, |owner_projects_list| owner_projects_list.retain(|h| h != &project_hash));

            // remove project from owner
            <ProjectHashOwner<T>>::remove(project_hash);

            // remove status record
            <ProjectHashStatus<T>>::remove(project_hash);

            // record the fact of deletion by whom
            <DeletedProjects<T>>::mutate(&project_hash, |deleted_project| deleted_project.push(deleted_project_struct));

            Self::deposit_event(RawEvent::ProjectDeleted(project_hash, project_owner, changed_by, project_status));

            Ok(())
        }

        fn reassign_project(origin, new_owner: T::AccountId, project_hash: ProjectHash) -> Result {
            ensure!(<ProjectHashStatus<T>>::exists(&project_hash), "The project does not exist!");

            // get project owner from hash
            let project_owner: T::AccountId = Self::project_hash_owner(&project_hash).ok_or("Error fetching project owner")?;

            let changer: T::AccountId = ensure_signed(origin)?;
            let mut changed_by: T::AccountId = changer.clone();

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise only the owner can change the data
            ensure!(project_owner == changer, "You cannot reassign a project you do not own");

            // retain all other projects except the one we want to reassign
            <OwnerProjectsList<T>>::mutate(&project_owner, |owner_projects_list| owner_projects_list.retain(|h| h != &project_hash));

            // Set new owner for hash
            <ProjectHashOwner<T>>::insert(&project_hash, &new_owner);
            <OwnerProjectsList<T>>::mutate(&new_owner, |owner_projects_list| owner_projects_list.push(project_hash));

            Self::deposit_event(RawEvent::ProjectReassigned(project_hash, new_owner, changed_by));

            Ok(())

        }

        fn close_project(origin, project_hash: ProjectHash) -> Result {
            ensure!(<ProjectHashStatus<T>>::exists(&project_hash), "The project does not exist!");

            let changer = ensure_signed(origin)?;

           // get project owner by hash
            let project_owner: T::AccountId = Self::project_hash_owner(&project_hash).ok_or("Error fetching project owner")?;

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise onlu the owner can change the data
            ensure!(project_owner == changer, "You cannot close a project you do not own");
            let project_status: ProjectStatus = 2;
            <ProjectHashStatus<T>>::insert(&project_hash, &project_status);

            Self::deposit_event(RawEvent::ProjectChanged(project_hash, changer, project_status));

            Ok(())
        }

        fn reopen_project(origin, project_hash: ProjectHash) -> Result {
            // Can only reopen a project that is in status "closed"
            let project_status: ProjectStatus = match Self::project_hash_status(&project_hash) {
                Some(2) => 1,
                _ => return Err("Project has the wrong status to be changed"),
                None => return Err("Project has no status"),
            };
            // ensure!(<ProjectHashStatus<T>>::exists(&project_hash), "The project has no status!");

            let changer = ensure_signed(origin)?;

            // get project owner by hash
            let project_owner: T::AccountId = Self::project_hash_owner(&project_hash).ok_or("Error fetching project owner")?;

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise only the owner can change the data
            ensure!(project_owner == changer, "You cannot change a project you do not own");

            <ProjectHashStatus<T>>::insert(&project_hash, &project_status);

            Self::deposit_event(RawEvent::ProjectChanged(project_hash, changer, project_status));

            Ok(())
        }

        fn end_or_unend_project(origin, project_hash: ProjectHash, project_status: ProjectStatus, state_change: bool) -> Result {
            ensure!(<ProjectHashStatus<T>>::exists(&project_hash), "The project does not exist!");

            let changer = ensure_signed(origin)?;

            // get project owner by hash
            let project_owner: T::AccountId = Self::project_hash_owner(&project_hash).ok_or("Error fetching project owner")?;

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise only the owner can change the data
            ensure!(project_owner == changer, "You cannot change a project you do not own");

            let mut new_project_status: ProjectStatus;
            // check if state change is re-opening
            // state_change = true, then it is ending/closing a project
            // state_change = reopening, then it is re-opening/unending a project

            match state_change {
                true => {
                    new_project_status = match Self::project_hash_status(&project_hash) {
                        Some(0) => { // project is open
                            match project_status {
                                3 => project_status, // on-hold
                                4 => project_status, // abandoned
                                5 => project_status, // cancelled
                                _ => return Err("Current state prevents setting new state."),
                            }
                        },
                        Some(1) => { //project reopened
                            match project_status {
                                3 => project_status, // on-hold
                                4 => project_status, // abandoned
                                5 => project_status, // cancelled
                                _ => return Err("Current state prevents setting new state."),
                            }
                        },
                        _ => return Err("Project cannot be set to closed."), // all other project states
                        None => return Err("Project has no status"), // some error
                    }
                },
                false => {
                    // Can only reopen a project that is in status "closed" or "on-hold"
                    new_project_status = match Self::project_hash_status(&project_hash) {
                        Some(2) => { // project closed can be reopened
                            match project_status {
                                1 => project_status, // set status to 1
                                _ => return Err("existing status cannot be reopened!"),
                            }
                        },
                        Some(3) => { // project was on-hold can be reopened
                            match project_status {
                                1 => project_status, // set status to 1
                                _ => return Err("existing status cannot be reopened!"),
                            }
                        },
                        _ => return Err("Project has the wrong status. Cannot be reopened"),
                        None => return Err("Project has no status"),
                    }
                }
            };

            <ProjectHashStatus<T>>::insert(&project_hash, &new_project_status);

            Self::deposit_event(RawEvent::ProjectChanged(project_hash, changer, new_project_status));

            Ok(())
        }

        // TODO Refactor to a single function for status change on projects
        // incorporate open(0), re-open(1), closed(2), abandoned(3), on-hold(4), cancelled(5), deleted(99) in refactoring.
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
    {
        ProjectRegistered(ProjectHash, AccountId),
        ProjectDeleted(ProjectHash, AccountId, AccountId, ProjectStatus),
        ProjectReassigned(ProjectHash, AccountId, AccountId),
        ProjectChanged(ProjectHash, AccountId, ProjectStatus),
    }
);

// functions that are called externally to check values internal to this module.
impl<T: Trait> Module<T> {
    pub fn check_owner_valid_project(owner: T::AccountId, project_hash: ProjectHash) -> bool {
        // set default return value
        let mut valid: bool = false;
        let project_owner = owner;

        // check validity of project
        if let true = Self::check_valid_project(project_hash.clone()) {
            match Self::project_hash_owner(project_hash) {
                Some(project_owner) => valid = true,
                None => return valid,
            }
        }

        return valid;
    }

    pub fn check_valid_project(project_hash: ProjectHash) -> bool {
        // set default return value
        let mut valid: bool = false;

        // check that the status of the project exists and is open or reopened.
        match Self::project_hash_status(&project_hash) {
            Some(0) => valid = true,
            Some(1) => valid = true,
            _ => return valid,
            None => return valid,
        }

        return valid;
    }
}

/// tests for this module
#[cfg(test)]
mod tests {
    use super::*;

    use primitives::{Blake2Hasher, H256};
    use runtime_io::with_externalities;
    use runtime_primitives::{
        testing::{Digest, DigestItem, Header},
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage,
    };
    use support::{assert_ok, impl_outer_origin};

    impl_outer_origin! {
        pub enum Origin for Test {}
    }

    // For testing the module, we construct most of a mock runtime. This means
    // first constructing a configuration type (`Test`) which `impl`s each of the
    // configuration traits of modules we want to use.
    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;
    impl system::Trait for Test {
        type Origin = Origin;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type Digest = Digest;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = ();
        type Log = DigestItem;
    }
    impl Trait for Test {
        type Event = ();
    }
    type ProjectModule = Module<Test>;

    // This function basically just builds a genesis storage key/value store according to
    // our desired mockup.
    fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
        system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap()
            .0
            .into()
    }

    // #[test]
    // fn it_works_for_default_value() {
    // 	with_externalities(&mut new_test_ext(), || {
    // 		assert_ok!(ProjectModule::do_something(Origin::signed(1), 42));
    // 		assert_eq!(ProjectModule::something(), Some(42));
    // 	});
    // }
}
