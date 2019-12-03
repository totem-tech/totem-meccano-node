// Copyright 2019 Chris D'Costa
// This file is part of Totem Live Accounting.
// Author Chris D'Costa email: chris.dcosta@totemaccounting.com

// Totem is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Totem is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Totem.  If not, see <http://www.gnu.org/licenses/>.

use support::{
    decl_event, decl_module, decl_storage, dispatch::Result, ensure, StorageMap, StorageValue,
};
use system::ensure_signed;

use parity_codec::{Decode, Encode};
use runtime_primitives::traits::Hash;
// use system::{self, ensure_signed};
use rstd::prelude::*;

// Totem crates
use crate::projects;

pub trait Trait: projects::Trait + system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// from Projects module
pub type StatusOfProject = projects::ProjectStatus; // open(0), re-open(1), closed(2), abandoned(3), on-hold(4), cancelled(5), deleted(99)
pub type ProjectHashRef = projects::ProjectHash;

pub type NumberOfBlocks = u64; // Quantity of blocks determines the passage of time
pub type StartOrEndBlockNumber = NumberOfBlocks;
pub type StatusOfTimeRecord = u16; // submitted(0), accepted(1), rejected(2), disputed(3), blocked(4), invoiced(5), reason_code(0), reason text.
pub type PostingPeriod = u16; // Not calendar period, but fiscal periods 1-15 (0-14)
pub type AcceptAssignedStatus = bool; // (true/false)
pub type LockStatus = bool; // Locked true, unlocked false
pub type ReasonCode = u16; // Reason for status change (TODO codes to be defined)
pub type ReasonCodeType = u16; // Category of reason code (TODO categories to be defined)
                               // pub type ReasonCodeText = Vec<u8>; // Reason for status change in text (not on chain!)
pub type BanStatus = bool; // Ban status (default is false)

// Tuple for reason code changes
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ReasonCodeStruct(ReasonCode, ReasonCodeType);

// Tuple for status code changes
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct BannedStruct(BanStatus, ReasonCodeStruct);

// This is the individual time record
#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Timekeeper<
    AccountId,
    ProjectHashRef,
    NumberOfBlocks,
    LockStatus,
    StatusOfTimeRecord,
    ReasonCodeStruct,
    PostingPeriod,
    StartOrEndBlockNumber,
> {
    pub worker: AccountId,
    pub project_hash: ProjectHashRef,
    pub total_blocks: NumberOfBlocks,
    pub locked_status: LockStatus,
    pub locked_reason: ReasonCodeStruct,
    pub submit_status: StatusOfTimeRecord,
    pub reason_code: ReasonCodeStruct,
    pub posting_period: PostingPeriod,
    pub start_block: StartOrEndBlockNumber,
    pub end_block: StartOrEndBlockNumber,
}

// It is recognised that measurements of time periods using block numbers as a timestamp is not the recommended approach
// due to significant time-drift over long periods of elapsed time.

// This module however uses number of blocks as a time measurement (with 1 block equivalent to approximately 5 seconds)
// on the basis that the employee's working time measurement segments do not present a
// significant calculation risk when measuring and capturing relatively small amounts of booked time.
// The blocktime therefore behaves similar to a stopwatch for timekeeping.

// It should be noted that validators timestamp each new block with the "correct" timestamp, which can be retrieved
// when needed to provide time analysis for accounting entries.

decl_storage! {
    trait Store for Module<T: Trait> as TimekeepingModule {
        // Project owner sends project ref to worker address (AccountId is the Worker).
        // Note: Currently unbounded Vec!

        // This is  a list of the Projects that are currently assigned by a project owner.
        // The worker can accept to work on these, or remove them from the list.
        // If they have already worked on them they cannot be removed.
        WorkerProjectsBacklogList get(worker_projects_backlog_list): map T::AccountId => Vec<ProjectHashRef>;
        // Accepted Status is true/false
        WorkerProjectsBacklogStatus get(worker_projects_backlog_status): map (ProjectHashRef, T::AccountId) => Option<AcceptAssignedStatus>;

        // List of all workers (team) booking time on the project
        // Used mainly by the Project owner, but other workers can be seen.
        // The two here will logically replace the above two storage items, however as much of the code is dependent on the status
        // there will have to be a re-write.
        // Note: Currently unbounded Vec!
        ProjectInvitesList get(project_invites_list): map ProjectHashRef => Vec<T::AccountId>;
        ProjectWorkersList get(project_workers_list): map ProjectHashRef => Vec<T::AccountId>;

        // project worker can be banned by project owner.
        // NOTE Project owner should not ban itself!!
        ProjectWorkersBanList get(project_workers_ban_list): map (ProjectHashRef, T::AccountId) => Option<BannedStruct>;

        // When did the project first book time (blocknumber = first seen block number)
        // maybe this should be moved to the projects.rs file?
        ProjectFirstSeen get(project_first_seen): map ProjectHashRef => Option<StartOrEndBlockNumber>;

        // This stores the total number of blocks (blocktime) for a given project.
        // It collates all time by all team members.
        TotalBlocksPerProject get(total_blocks_per_project): map ProjectHashRef => Option<NumberOfBlocks>;

        // This records the total amount of blocks booked per address (worker), per project.
        // It records the first seen block which indicates when the project worker first worked on the project
        // It also records the total time (number of blocks) for that address
        TotalBlocksPerProjectPerAddress get(total_blocks_per_project_per_address): map (T::AccountId,ProjectHashRef) => Option<NumberOfBlocks>;

        // overall hours worked on all projects for a given address for all projects
        TotalBlocksPerAddress get(total_blocks_per_address): map T::AccountId => Option<NumberOfBlocks>;

        // Time Record Hashes created by submitter
        // Unbounded! TODO
        WorkerTimeRecordsHashList get(worker_time_records_hash_list): map T::AccountId => Vec<T::Hash>;
        TimeHashOwner get(time_hash_owner): map T::Hash => Option<T::AccountId>;

        // All the time records for a given project
        // Unbounded! TODO
        ProjectTimeRecordsHashList get(project_time_records_hash_list): map ProjectHashRef => Vec<T::Hash>;

        // This records the amount of blocks per address, per project, per entry. // start block number can be calculated. Only accepted if an end block number is given in the transaction as this is the "service rendered" date for accounting purposes.
        //    .map(Address, Project Hash, End Block number => number of blocks, StatusOfTimeRecors (submitted, accepted, rejected, disputed, blocked, invoiced, locked, reason_code, reason text.), posting-period)
        TimeRecord get(time_record): map T::Hash => Option<Timekeeper<T::AccountId,ProjectHashRef,NumberOfBlocks,LockStatus,StatusOfTimeRecord,ReasonCodeStruct,PostingPeriod,StartOrEndBlockNumber>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;
        // Project owner invites worker to project
        fn notify_project_worker(origin, worker: T::AccountId, project_hash: ProjectHashRef) -> Result {
            let who = ensure_signed(origin)?;

            // check project hash exists and is owner by sender
            let hash_has_correct_owner = <projects::Module<T>>::check_owner_valid_project(who.clone(), project_hash.clone());
            ensure!(hash_has_correct_owner, "Invalid project or project owner is not correct");

            // ensure that the project has not already been assigned to the worker, and that they have accepted already
            let status_tuple_key = (project_hash.clone(), worker.clone());

            match Self::worker_projects_backlog_status(&status_tuple_key) {
                Some(true) => return Err("Worker already accepted the project."),
                Some(false) => return Err("Worker already assigned the project, but hasn't formally accepted."),
                None => (),  // OK this project has not been assigned yet.
            };

            // The initial status of the acceptance to work on the project
            let accepted_status: AcceptAssignedStatus = false;

           // Adds project to list of projects assigned to worker address
           // Worker does not therefore need to be notified of new project assigned to them, as it will appear in
           // a list of projects
           <WorkerProjectsBacklogList<T>>::mutate(&worker, |worker_projects_backlog_list| worker_projects_backlog_list.push(project_hash.clone()));

           // set initial status
           <WorkerProjectsBacklogStatus<T>>::insert(&status_tuple_key, accepted_status);

            // add worker to project team invitations, pending acceptance.
            <ProjectInvitesList<T>>::mutate(&project_hash, |project_invites_list| {
                project_invites_list.push(worker.clone())
            });

            // issue event
            Self::deposit_event(RawEvent::NotifyProjectWorker(worker, project_hash));
            Ok(())
        }

        fn worker_acceptance_project(origin, project_hash: ProjectHashRef, accepted: AcceptAssignedStatus) -> Result {
            let who = ensure_signed(origin)?;

            // check that this project is still active (not closed or deleted or with no status)
            ensure!(<projects::Module<T>>::check_valid_project(project_hash.clone()), "Project not active.");

            // check that the worker on this project is the signer
            if let worker_project = Self::worker_projects_backlog_list(&who)
                .into_iter()
                .find(| &x| x == project_hash.clone())
                .ok_or("This identity has not been assigned the project!")?
            {

            // Sets the new status of the acceptance to work on the project
            let status_tuple_key = (project_hash.clone(), who.clone());
                // Check that the project worker has accepted the project or rejected.
                match &accepted {
                    true => {
                        // let accepted_status: AcceptAssignedStatus = true;
                        match Self::worker_projects_backlog_status(&status_tuple_key) {
                            // Worker confirms acceptance of project assignment. This effectively is an agreement that
                            // the project owner will accept time bookings from the worker as long as the project is still active.
                            // Some(false) => Self::store_worker_acceptance(project_hash, who, accepted_status),
                            Some(false) => Self::store_worker_acceptance(project_hash, who, accepted),
                            Some(true) => return Err("Project worker has already accepted the project."),
                            None => return Err("Project worker has not been assigned to this project yet."),
                        };
                    },
                    false => {
                        match Self::worker_projects_backlog_status(&status_tuple_key) {
                            // Only allow remove if the worker has been assigned this project,
                            // and that the status is unaccepted.
                            Some(false) => {
                                // Worker is removing this acceptance status
                                <WorkerProjectsBacklogStatus<T>>::take(&status_tuple_key);

                                // Remove project assignment from list
                                <WorkerProjectsBacklogList<T>>::mutate(&who, |worker_projects_backlog_list| {
                                    worker_projects_backlog_list.retain(|h| h != &project_hash)
                                });
                            },
                            Some(true) => return Err("Cannot remove project that has been accepted already."),
                            None => return Err("Project worker has not been assigned to this project yet."),
                        };

                    }
                }

            };

            Ok(())
        }

        // Worker submits/resubmits time record
        fn submit_time(
            origin,
            project_hash: ProjectHashRef,
            input_time_hash: T::Hash,
            submit_status: StatusOfTimeRecord,
            reason_for_change: ReasonCodeStruct,
            locked_status: LockStatus,
            reason_for_lock: ReasonCodeStruct,
            number_of_blocks:  NumberOfBlocks,
            posting_period: PostingPeriod,
            start_block_number: StartOrEndBlockNumber,
            end_block_number: StartOrEndBlockNumber
                        ) -> Result {
            let who = ensure_signed(origin)?;

            // Check that this project is still active (not closed or deleted or with no status)
            ensure!(<projects::Module<T>>::check_valid_project(project_hash.clone()), "Project not active.");

            // Check worker is not on the banned list
            let ban_list_key = (project_hash.clone(), who.clone());
            ensure!(!<ProjectWorkersBanList<T>>::exists(&ban_list_key), "This worker is banned!");

            // Check worker is part of the team
            let check_team_member = who.clone();
            if let worker_ok = Self::project_workers_list(project_hash.clone())
                .into_iter()
                .find(| x| x == &check_team_member)
                .ok_or("This identity has not been assigned the project!")?
            {
                // For testing
                // let input_time_hash = hex!("e4d673a76e8b32ca3989dbb9f444f71813c88d36120170b15151d58c7106cc83");
                // let default_hash: T::Hash = hex!("e4d673a76e8b32ca3989dbb9f444f71813c88d36120170b15151d58c7106cc83");

                let default_bytes = "Default hash";
                let default_hash: T::Hash = T::Hashing::hash(&default_bytes.encode().as_slice()); // default hash BlakeTwo256

                // set default lock and reason code and type default values
                let mut initial_submit_reason = ReasonCodeStruct(0, 0);
                let mut initial_reason_for_lock = ReasonCodeStruct(0, 0);

                // check that the submission is using either the default hash or some other hash.
                if let default_hash = input_time_hash {

                    // This is the default hash therefore it is a new submission.
                    // Create a new random hash
                    let time_hash: T::Hash = <system::Module<T>>::random_seed().using_encoded(<T as system::Trait>::Hashing::hash);

                    // prepare new time key
                    // let time_key = (who.clone(), project_hash.clone(), time_hash.clone());
                    let time_key = time_hash.clone();

                    // prepare time record
                    let time_data: Timekeeper<
                                        T::AccountId,
                                        ProjectHashRef,
                                        NumberOfBlocks,
                                        LockStatus,
                                        StatusOfTimeRecord,
                                        ReasonCodeStruct,
                                        PostingPeriod,
                                        StartOrEndBlockNumber> = Timekeeper {
                                            worker: who.clone(),
                                            project_hash: project_hash.clone(),
                                            total_blocks: number_of_blocks.into(),
                                            locked_status: false,
                                            locked_reason: initial_reason_for_lock,
                                            submit_status: 0,
                                            reason_code: initial_submit_reason,
                                            posting_period: 0, // temporary for this version.
                                            start_block: start_block_number.into(),
                                            end_block: end_block_number.into()
                                        };

                    // Now update all time relevant records

                    //WorkerTimeRecordsHashList
                    <WorkerTimeRecordsHashList<T>>::mutate(&who, |worker_time_records_hash_list| worker_time_records_hash_list.push(time_hash.clone()));

                    // Add time hash to project list
                    <ProjectTimeRecordsHashList<T>>::mutate(&project_hash, |project_time_hash_list| {
                        project_time_hash_list.push(time_hash.clone())
                    });

                    //TimeHashOwner
                    <TimeHashOwner<T>>::insert(time_hash.clone(), who.clone());

                    // Insert record
                    <TimeRecord<T>>::insert(time_key, &time_data);
                    Self::deposit_event(RawEvent::SubmitedTimeRecord(time_hash));
                    
                } else { // This is not the default hash, therefore it is a resubmission, and should have a new status code.
                    
                    // prepare new time key
                    let original_time_key = input_time_hash.clone();
                    
                    // Check this is an existing time record
                    // and get the details using the resubmitted hash
                    let old_time_record = Self::time_record(&original_time_key).ok_or("Time record does not exist, or this is not from the worker.")?;
                    ensure!(!old_time_record.locked_status, "You cannot change a locked time record!");
                    
                    let proposed_new_status = submit_status.clone();

                    // prepare new time record.
                    let mut new_time_data: Timekeeper<T::AccountId,ProjectHashRef,NumberOfBlocks,LockStatus,StatusOfTimeRecord,ReasonCodeStruct,PostingPeriod,StartOrEndBlockNumber> = Timekeeper {
                        worker: who.clone(),
                        project_hash: project_hash.clone(),
                        total_blocks: number_of_blocks.into(),
                        locked_status: false,
                        locked_reason: initial_reason_for_lock,
                        submit_status: 0,
                        reason_code: initial_submit_reason,
                        posting_period: 0,
                        start_block: start_block_number.into(),
                        end_block: end_block_number.into()
                    };
                    
                    // Possible states are
                    // draft(0), 
                    // submitted(1), 
                    // disputed(100), can be resubmitted, if the current status is < 100 return this state
                    // rejected(200), can be resubmitted, if the current status is < 100 return this state
                    // accepted(300), can no longer be rejected or disputed, > 200 < 400
                    // invoiced(400), can no longer be rejected or disputed, > 300 < 500
                    // blocked(999), 
                    
                    // Submit
                    // project owner disputes, setting the state to 100... 100 can only be set if the current status is 0
                    // project owner rejects, setting the state to 200... 200 can only be set if the current status is 0
                    // Worker can resubmit time setting it back to 0... 0 can only be set if the current status < 300

                    // project owner accepts time setting status to 300... 300 can only be set if the current status is 0 or 400 - a worker can invoice before acceptance
                    // Project worker makes invoice. Worker can only create invoice if the current status is 0 or 300. 
                    
                    // project owner response window expires 

                    match old_time_record.submit_status {
                        0 => {
                            match proposed_new_status {
                                0 => (), // changing an already sumitted record. OK, do nothing.
                                1 => {new_time_data.submit_status = proposed_new_status}, // Draft to submitted.
                                // not appropriate to set these codes here. Other specific functions exist.
                                _ => return Err("This status has not been implemented or is not to be set this way."),
                            }
                        },
                        100 | 200 => {
                            // The existing record is rejected or disputed. The sender is therefore attempting to change the
                            // record. Only the worker can change the record.
                            // Ensure that the sender is the owner of the time record
                            ensure!({old_time_record.worker == new_time_data.worker}, "You cannot change a time record you do not own!");
                            ensure!({
                                old_time_record.total_blocks != new_time_data.total_blocks &&   
                                old_time_record.start_block != new_time_data.start_block &&   
                                old_time_record.end_block != new_time_data.end_block   
                            }, "Nothing has changed! Record will not be updated.");
                            
                            // TODO remove any submitted reason codes.
                            // 0, 0 initial reason code is the default

                            // No matter what the submlitted value as long as the owner is making the change
                            // the submit_status is already set to 0. No further processing necessary.
                            
                        },
                        300 => {
                            // The project owner has already accepted, but a correction is agreed with worker. 
                            // therefore reset the record to "draft"
                            let hash_has_correct_owner = <projects::Module<T>>::check_owner_valid_project(who.clone(), project_hash.clone());
                            ensure!(hash_has_correct_owner, "Invalid project or project owner is not correct");
                            
                            // ensure that a correct reason is given by project owner
                            // TODO inspect reason code values, change if necessary
                            
                            // force change pending above
                            // [1, 1] = [time record can be re-edited by the team member, set in time module]
                            new_time_data.reason_code = ReasonCodeStruct(1, 1); 
                            
                            // No matter what the submlitted value as long as the owner is making the change
                            // the submit_status is already set to 0. No further processing necessary.
                        },
                        400 => return Err("Time record already invoiced. It cannot be changed."),
                        999 => return Err("Time has been blocked by Project Owner. Check the reason code."),
                        _ => return Err("This should not occur. Your time record has an invalid Status Code"),
                    };

                    Self::update_time_record(original_time_key, new_time_data);
                    
                };
            } // no explicit else needed.

            Ok(())
        }

        // Project owner sets authorisation status of time record
        fn authorise_time(
            origin,
            worker: T::AccountId,
            project_hash: ProjectHashRef,
            input_time_hash: T::Hash,
            status_of_record: StatusOfTimeRecord,
            locked: LockStatus,
            reason: ReasonCodeStruct
            ) -> Result {
            let who = ensure_signed(origin)?;

            // ensure that the caller is the project owner
            let hash_has_correct_owner = <projects::Module<T>>::check_owner_valid_project(who.clone(), project_hash.clone());
            ensure!(hash_has_correct_owner, "Invalid project or project owner is not correct");

            // prepare new time key
            let original_time_key = input_time_hash.clone();
            
            // Check this is an existing time record
            // and get the details using the resubmitted hash
            let mut changing_time_record = Self::time_record(&original_time_key).ok_or("Time record does not exist, or this is not from the worker.")?;
            ensure!(!changing_time_record.locked_status, "You cannot change a locked time record!");
            
            let proposed_new_status = status_of_record.clone();

            match changing_time_record.submit_status {
                0 => return Err("Time record has not been finalised by worker."), 
                1 => {
                    match proposed_new_status {
                        0 | 400 => return Err("Project owner cannot set this status for the  time record."), // changing an already sumitted record. OK, do nothing.
                        100 | 200 | 300 | 999  => {
                            // Record is being disputed or rejected or accepted of blocked by project owner

                            // ensure that a correct reason is given by project owner
                            // TODO inpect reason code values
                            // new_time_data.reason_code = ReasonCodeStruct(1, 1); 
                            
                            changing_time_record.submit_status = proposed_new_status;
                        }, 
                        _ => return Err("This status has not been implemented"),
                    }
                }
                // The existing record is in a state that cannot be changed by the project owner. 
                100 | 200 | 300 | 400 | 999 => return Err("The project cannot be changed by the project owner anymore."),
                _ => return Err("This should not occur. The stored time record has an invalid Status Code"),
            };
            
            // If project has not ever been seen before and time has not been booked then
            // check if record start blocknumber is lower than currently stored value. If so, replace.
            // this is in the event that the project owner initially approves a time record that was later than
            // a subsequent time record.
            if <ProjectFirstSeen<T>>::exists(&changing_time_record.project_hash) && Self::project_first_seen(&changing_time_record.project_hash) > Some(changing_time_record.start_block) {                
                // Remove existing record
                <ProjectFirstSeen<T>>::take(&changing_time_record.project_hash);
                // insert new record
                <ProjectFirstSeen<T>>::insert(&changing_time_record.project_hash, changing_time_record.start_block);
                
            } else {
                <ProjectFirstSeen<T>>::insert(&changing_time_record.project_hash, changing_time_record.start_block);
            };
            
            Self::update_time_record(original_time_key, changing_time_record);    
            Self::deposit_event(RawEvent::SetAuthoriseStatus(who));

            Ok(())
        }
        // Worker invoices the time record
        fn invoice_time(
            origin,
            project_hash: ProjectHashRef,
            input_time_hash: T::Hash
        ) -> Result {
            let who = ensure_signed(origin)?;
            // TODO This is normally set by the invoice module not by the time module
            // This needs to be reviewed once the invoice module is being developed.
            // Could be that this calls a function from within the invoice module.
            // can only invoice when time is accepted

            // Set StatusOfTimeRecord
            // invoiced,
            Self::deposit_event(RawEvent::InvoiceTime(who));
            Ok(())
        }

        // Project owner pays invoice
        fn pay_time(
            origin,
            project_hash: ProjectHashRef,
            input_time_hash: T::Hash
        ) -> Result {
            let who = ensure_signed(origin)?;




            Self::deposit_event(RawEvent::PayTime(who.clone()));
            // Self::lock_time_record(who.clone(), project_hash.clone(), input_time_hash.clone());
            Self::deposit_event(RawEvent::LockTimeRecord());
            Ok(())
        }

        // Full payment triggers locked record
        fn lock_time_record(
            origin,
            project_hash: ProjectHashRef,
            input_time_hash: T::Hash
        ) -> Result {

            Self::deposit_event(RawEvent::LockTimeRecord());
            Ok(())
        }

        // Full payment triggers locked record
        fn unlock_time_record(
            origin,
            project_hash: ProjectHashRef,
            input_time_hash: T::Hash
        ) -> Result {


            Self::deposit_event(RawEvent::UnLockTimeRecord());
            Ok(())
        }

        // Full payment triggers locked record
        fn ban_worker(
            origin,
            project_hash: ProjectHashRef,
            worker: T::AccountId
        ) -> Result {
            // check that you are not banning is not yourself!

            Self::deposit_event(RawEvent::Banned());
            Ok(())
        }

        // Full payment triggers locked record
        fn unban_worker(
            origin,
            project_hash: ProjectHashRef,
            worker: T::AccountId
        ) -> Result {

            Self::deposit_event(RawEvent::UnBanned());
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    fn store_worker_acceptance(
        project_hash: ProjectHashRef,
        who: T::AccountId,
        accepted_status: AcceptAssignedStatus,
    ) {
        let mut stored_ok: bool = false;
        let status_tuple_key = (project_hash.clone(), who.clone());
        // add worker to project team
        <ProjectWorkersList<T>>::mutate(&project_hash, |project_workers_list| {
            project_workers_list.push(who.clone())
        });

        // Remove from invitations list
        <ProjectInvitesList<T>>::mutate(&project_hash, |project_invites_list| {
            project_invites_list.retain(|h| h != &who)
        });


        // set new status
        <WorkerProjectsBacklogStatus<T>>::insert(status_tuple_key, &accepted_status);

        // remove from backlog
        <WorkerProjectsBacklogList<T>>::mutate(&who, |worker_projects_backlog_list| {
            worker_projects_backlog_list.retain(|h| h != &project_hash)
        });

        // issue event
        Self::deposit_event(RawEvent::WorkerAcceptanceStatus(
            who,
            project_hash,
            accepted_status,
        ));
    }

    fn update_time_record(
        // (a, b, c): (T::AccountId, ProjectHashRef, T::Hash),
        k: T::Hash,
        d: Timekeeper<
            T::AccountId,
            ProjectHashRef,
            NumberOfBlocks,
            LockStatus,
            StatusOfTimeRecord,
            ReasonCodeStruct,
            PostingPeriod,
            StartOrEndBlockNumber,
        >,        
    ) {
        // let time_record_key = (\a.clone(), b.clone(), c.clone());
        let time_record_key = k.clone();

        // remove existing record (if one exists)
        <TimeRecord<T>>::take(&time_record_key);        
        
        // store new time record
        <TimeRecord<T>>::insert(&time_record_key, d);

        // issue event
        // Self::deposit_event(RawEvent::SubmitedTimeRecord(a, b, c));
        Self::deposit_event(RawEvent::SubmitedTimeRecord(k));
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
        Hash = <T as system::Trait>::Hash,
    {
        SubmitedTimeRecord(Hash),
        NotifyProjectWorker(AccountId, ProjectHashRef),
        WorkerAcceptanceStatus(AccountId, ProjectHashRef, AcceptAssignedStatus),
        SetAuthoriseStatus(AccountId),
        InvoiceTime(AccountId),
        PayTime(AccountId),
        LockTimeRecord(),
        UnLockTimeRecord(),
        Banned(),
        UnBanned(),
    }
);

// tests for this module
// #[cfg(test)]
// mod tests {
// 	use super::*;

// 	use runtime_io::with_externalities;
// 	use primitives::{H256, Blake2Hasher};
// 	use support::{impl_outer_origin, assert_ok};
// 	use runtime_primitives::{
// 		BuildStorage,
// 		traits::{BlakeTwo256, IdentityLookup},
// 		testing::{Digest, DigestItem, Header}
// 	};

// 	impl_outer_origin! {
// 		pub enum Origin for Test {}
// 	}

// 	// For testing the module, we construct most of a mock runtime. This means
// 	// first constructing a configuration type (`Test`) which `impl`s each of the
// 	// configuration traits of modules we want to use.
// 	#[derive(Clone, Eq, PartialEq)]
// 	pub struct Test;
// 	impl system::Trait for Test {
// 		type Origin = Origin;
// 		type Index = u64;
// 		type BlockNumber = u64;
// 		type Hash = H256;
// 		type Hashing = BlakeTwo256;
// 		type Digest = Digest;
// 		type AccountId = u64;
// 		type Lookup = IdentityLookup<Self::AccountId>;
// 		type Header = Header;
// 		type Event = ();
// 		type Log = DigestItem;
// 	}
// 	impl Trait for Test {
// 		type Event = ();
// 	}
// 	type TimekeepingModule = Module<Test>;

// 	// This function basically just builds a genesis storage key/value store according to
// 	// our desired mockup.
// 	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
// 		system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
// 	}

// 	#[test]
// 	fn it_works_for_default_value() {
// 		with_externalities(&mut new_test_ext(), || {
// 			assert_ok!(TimekeepingModule::do_something(Origin::signed(1), 42));
// 			assert_eq!(TimekeepingModule::something(), Some(42));
// 		});
// 	}
// }
