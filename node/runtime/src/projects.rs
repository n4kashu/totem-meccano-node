//!                              Næ§@@@ÑÉ©
//!                        æ@@@@@@@@@@@@@@@@@@
//!                    Ñ@@@@?.?@@@@@@@@@@@@@@@@@@@N
//!                 ¶@@@@@?^%@@.=@@@@@@@@@@@@@@@@@@@@
//!               N@@@@@@@?^@@@»^@@@@@@@@@@@@@@@@@@@@@@
//!               @@@@@@@@?^@@@».............?@@@@@@@@@É
//!              Ñ@@@@@@@@?^@@@@@@@@@@@@@@@@@@'?@@@@@@@@Ñ
//!              @@@@@@@@@?^@@@»..............»@@@@@@@@@@
//!              @@@@@@@@@?^@@@»^@@@@@@@@@@@@@@@@@@@@@@@@
//!              @@@@@@@@@?^ë@@&.@@@@@@@@@@@@@@@@@@@@@@@@
//!               @@@@@@@@?^´@@@o.%@@@@@@@@@@@@@@@@@@@@©
//!                @@@@@@@?.´@@@@@ë.........*.±@@@@@@@æ
//!                 @@@@@@@@?´.I@@@@@@@@@@@@@@.&@@@@@N
//!                  N@@@@@@@@@@ë.*=????????=?@@@@@Ñ
//!                    @@@@@@@@@@@@@@@@@@@@@@@@@@@¶
//!                        É@@@@@@@@@@@@@@@@Ñ¶
//!                             Næ§@@@ÑÉ©

//! Copyright 2020 Chris D'Costa
//! This file is part of Totem Live Accounting.
//! Author Chris D'Costa email: chris.dcosta@totemaccounting.com

//! Totem is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.

//! Totem is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.

//! You should have received a copy of the GNU General Public License
//! along with Totem.  If not, see <http://www.gnu.org/licenses/>.

use parity_codec::{Decode, Encode};
use rstd::prelude::*;
use support::{decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageMap};
use system::{self, ensure_signed};

// Totem traits
use crate::projects_traits::{ Validating };

pub type ProjectStatus = u16; // Reference supplied externally

#[derive(PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct DeletedProject<AccountId, ProjectStatus> {
    pub owned_by: AccountId,
    pub deleted_by: AccountId,
    pub status: ProjectStatus,
}

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as ProjectModule {
        ProjectHashStatus get(project_hash_status): map T::Hash => Option<ProjectStatus>;
        DeletedProjects get(deleted_project): map T::Hash => Vec<DeletedProject<T::AccountId, ProjectStatus>>;
        ProjectHashOwner get(project_hash_owner): map T::Hash => Option<T::AccountId>;
        OwnerProjectsList get(owner_projects_list): map T::AccountId => Vec<T::Hash>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;

        fn add_new_project(origin, project_hash: T::Hash) -> Result {

            // Check that the project does not exist
            ensure!(!<ProjectHashStatus<T>>::exists(project_hash.clone()), "The project already exists!");

            // Check that the project was not deleted already
            ensure!(!<DeletedProjects<T>>::exists(project_hash.clone()), "The project was already deleted!");

            // proceed to store project
            let who = ensure_signed(origin)?;
            let project_status: ProjectStatus = 0;

            // TODO limit nr of Projects per Account.
            <ProjectHashStatus<T>>::insert(project_hash.clone(), &project_status);
            <ProjectHashOwner<T>>::insert(project_hash.clone(), &who);
            <OwnerProjectsList<T>>::mutate(&who, |owner_projects_list| owner_projects_list.push(project_hash.clone()));

            Self::deposit_event(RawEvent::ProjectRegistered(project_hash, who));

            Ok(())
        }

        fn remove_project(origin, project_hash: T::Hash) -> Result {
            ensure!(<ProjectHashStatus<T>>::exists(project_hash.clone()), "The project does not exist!");

            // get project by hash
            let project_owner: T::AccountId = Self::project_hash_owner(project_hash.clone()).ok_or("Error fetching project owner")?;

            // check transaction is signed.
            let changer: T::AccountId = ensure_signed(origin)?;

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise only the owner can change the data
            ensure!(project_owner == changer, "You cannot delete a project you do not own");

            let changed_by: T::AccountId = changer.clone();

            let deleted_project_struct = DeletedProject {
                owned_by: project_owner.clone(),
                deleted_by: changed_by,
                status: 999
            };

            // retain all other projects except the one we want to delete
            <OwnerProjectsList<T>>::mutate(&project_owner, |owner_projects_list| owner_projects_list.retain(|h| h != &project_hash));

            // remove project from owner
            <ProjectHashOwner<T>>::remove(project_hash.clone());

            // remove status record
            <ProjectHashStatus<T>>::remove(project_hash.clone());

            // record the fact of deletion by whom
            <DeletedProjects<T>>::mutate(project_hash.clone(), |deleted_project| deleted_project.push(deleted_project_struct));

            Self::deposit_event(RawEvent::ProjectDeleted(project_hash, project_owner, changer, 999));

            Ok(())
        }

        fn reassign_project(origin, new_owner: T::AccountId, project_hash: T::Hash) -> Result {
            ensure!(<ProjectHashStatus<T>>::exists(project_hash.clone()), "The project does not exist!");

            // get project owner from hash
            let project_owner: T::AccountId = Self::project_hash_owner(project_hash.clone()).ok_or("Error fetching project owner")?;

            let changer: T::AccountId = ensure_signed(origin)?;
            let changed_by: T::AccountId = changer.clone();

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise only the owner can change the data
            ensure!(project_owner == changer, "You cannot reassign a project you do not own");

            // retain all other projects except the one we want to reassign
            <OwnerProjectsList<T>>::mutate(&project_owner, |owner_projects_list| owner_projects_list.retain(|h| h != &project_hash));

            // Set new owner for hash
            <ProjectHashOwner<T>>::insert(project_hash.clone(), &new_owner);
            <OwnerProjectsList<T>>::mutate(&new_owner, |owner_projects_list| owner_projects_list.push(project_hash));

            Self::deposit_event(RawEvent::ProjectReassigned(project_hash, new_owner, changed_by));

            Ok(())

        }

        fn close_project(origin, project_hash: T::Hash) -> Result {
            ensure!(<ProjectHashStatus<T>>::exists(project_hash.clone()), "The project does not exist!");

            let changer = ensure_signed(origin)?;

           // get project owner by hash
            let project_owner: T::AccountId = Self::project_hash_owner(project_hash.clone()).ok_or("Error fetching project owner")?;

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise onlu the owner can change the data
            ensure!(project_owner == changer, "You cannot close a project you do not own");
            let project_status: ProjectStatus = 500;
            <ProjectHashStatus<T>>::insert(project_hash.clone(), &project_status);

            Self::deposit_event(RawEvent::ProjectChanged(project_hash, changer, project_status));

            Ok(())
        }

        fn reopen_project(origin, project_hash: T::Hash) -> Result {
            // Can only reopen a project that is in status "closed"
            let project_status: ProjectStatus = match Self::project_hash_status(project_hash.clone()) {
                Some(500) => 100,
                _ => return Err("Project has the wrong status to be changed"),
                // None => return Err("Project has no status"),
            };

            let changer = ensure_signed(origin)?;

            // get project owner by hash
            let project_owner: T::AccountId = Self::project_hash_owner(project_hash.clone()).ok_or("Error fetching project owner")?;

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise only the owner can change the data
            ensure!(project_owner == changer, "You cannot change a project you do not own");

            <ProjectHashStatus<T>>::insert(project_hash.clone(), &project_status);

            Self::deposit_event(RawEvent::ProjectChanged(project_hash, changer, project_status));

            Ok(())
        }

        fn set_status_project(origin, project_hash: T::Hash, project_status: ProjectStatus) -> Result {
            ensure!(<ProjectHashStatus<T>>::exists(project_hash.clone()), "The project does not exist!");

            let changer = ensure_signed(origin)?;

            // get project owner by hash
            let project_owner: T::AccountId = Self::project_hash_owner(project_hash.clone()).ok_or("Error fetching project owner")?;

            // TODO Implement a sudo for cleaning data in cases where owner is lost
            // Otherwise only the owner can change the data
            ensure!(project_owner == changer, "You cannot change a project you do not own");

            let current_project_status = Self::project_hash_status(project_hash.clone()).ok_or("Error fetching project status")?;
            // let proposed_project_status: ProjectStatus = project_status.clone();
            let proposed_project_status = project_status.clone();

            // Open	0
            // Reopen	100
            // On Hold	200
            // Abandon	300
            // Cancel	400
            // Close	500
            // Delete	999

            // Project owner creates project, set status to 0
            // Project owner puts on hold, setting the state to 200... 200 can only be set if the current status is  <= 101
            // Project owner abandons, setting the state to 300... 300 can only be set if the current status is  <= 101
            // Project owner cancels, setting the state to 400... 400 can only be set if the current status is  <= 101
            // Project owner close, setting the state to 500... 500 can only be set if the current status is  <= 101
            // Project owner reopen, setting the state to 100... 100 can only be set if the current status is  200 || 300 || 500
            // Project owner deletes, setting the state to 999... 999 cannot be set here.
            // Project owner other, setting the state to other value... cannot be set here.

                match current_project_status {
                    0 | 100 => {
                        // can set 200, 300, 400, 500
                        match proposed_project_status {
                            0 | 100  => return Err("The proposed project status is the same as the existing one."),
                            200 | 300 | 400 | 500  => (),
                            _ => return Err("The proposed project status cannot be applied to the current project status."),
                        };
                    },
                    200 | 300 | 500 => {
                        // only set 100
                        match proposed_project_status {
                            100  => (),
                            _ => return Err("The proposed project status cannot be applied to the current project status."),
                        };
                    },
                    _ => return Err("This proposed project status may not yet be implemented or is incorrect."),
                };

            let allowed_project_status: ProjectStatus =  proposed_project_status.into();

            <ProjectHashStatus<T>>::insert(project_hash.clone(), &allowed_project_status);

            Self::deposit_event(RawEvent::ProjectChanged(project_hash, changer, allowed_project_status));

            Ok(())
        }

    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
        Hash = <T as system::Trait>::Hash,
        ProjectStatus = u16,
    {
        ProjectRegistered(Hash, AccountId),
        ProjectDeleted(Hash, AccountId, AccountId, ProjectStatus),
        ProjectReassigned(Hash, AccountId, AccountId),
        ProjectChanged(Hash, AccountId, ProjectStatus),
    }
);

impl<T: Trait> Validating<T::AccountId,T::Hash> for Module<T> {
    fn is_project_owner(o: T::AccountId, h: T::Hash) -> bool {
        // set default return value
        let mut valid: bool = false;
        
        // check ownership of project
        match Self::project_hash_owner(h.clone()) {
            Some(owner) => {
                if o == owner {
                    valid = true;
                } else {
                    return valid;
                }
            },
            None => return valid,
        }
        
        return valid;
    }

    fn is_owner_and_project_valid(o: T::AccountId, h: T::Hash) -> bool {
        // set default return value
        let mut valid: bool = false;

        // check validity of project
        if let true = Self::is_project_valid(h.clone()) {
            match Self::project_hash_owner(h.clone()) {
                Some(owner) => {
                    if o == owner {
                        valid = true;
                    } else {
                        return valid;
                    }
                },
                None => return valid,
            }
        }

        return valid;
    }

    fn is_project_valid(h: T::Hash) -> bool {
        // set default return value
        let mut valid: bool = false;

        // check that the status of the project exists and is open or reopened.
        match Self::project_hash_status(h.clone()) {
            Some(0) | Some(100) => valid = true,
            _ => return valid,
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
