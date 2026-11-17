//! Roles, permissions and per-case access control.
//!
//! Enforced here, in the core, never in the UI. Two invariants matter most:
//! * the IT administrator manages users, backups and the machine, but cannot read case content;
//! * only a notary may finalise, number and sign off an act, approve an AML decision or close an
//!   accounting period.

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    NotaryTitular,
    NotaryAssociate,
    NotaryInterim,
    HeadClerk,
    Clerk,
    Trainee,
    Accountant,
    Inspector,
    ItAdmin,
}

impl Role {
    pub fn is_notary(self) -> bool {
        matches!(
            self,
            Role::NotaryTitular | Role::NotaryAssociate | Role::NotaryInterim
        )
    }

    pub fn code(self) -> &'static str {
        match self {
            Role::NotaryTitular => "notary_titular",
            Role::NotaryAssociate => "notary_associate",
            Role::NotaryInterim => "notary_interim",
            Role::HeadClerk => "head_clerk",
            Role::Clerk => "clerk",
            Role::Trainee => "trainee",
            Role::Accountant => "accountant",
            Role::Inspector => "inspector",
            Role::ItAdmin => "it_admin",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    CaseRead,
    CaseWrite,
    CaseCreate,
    CaseDelete,
    CaseAclManage,
    PartyRead,
    PartyWrite,
    DocumentRead,
    DocumentWrite,
    DocumentDelete,
    ActRead,
    ActDraft,
    ActFinalise,
    ActAmendFinalised,
    ActIssueCopy,
    RegisterRead,
    RegisterClose,
    AccountingRead,
    AccountingWrite,
    AccountingClose,
    AmlRead,
    AmlWrite,
    AmlApprove,
    StrExport,
    RulesRead,
    RulesWrite,
    RulesApprove,
    AuditRead,
    AuditVerify,
    BackupManage,
    BackupRestore,
    UserManage,
    SettingsRead,
    SettingsManage,
    ExportCreate,
    ExportApprove,
    ReportRead,
    InspectionExport,
    BreakGlass,
}

impl Permission {
    pub fn code(self) -> &'static str {
        match self {
            Permission::CaseRead => "case.read",
            Permission::CaseWrite => "case.write",
            Permission::CaseCreate => "case.create",
            Permission::CaseDelete => "case.delete",
            Permission::CaseAclManage => "case.acl.manage",
            Permission::PartyRead => "party.read",
            Permission::PartyWrite => "party.write",
            Permission::DocumentRead => "document.read",
            Permission::DocumentWrite => "document.write",
            Permission::DocumentDelete => "document.delete",
            Permission::ActRead => "act.read",
            Permission::ActDraft => "act.draft",
            Permission::ActFinalise => "act.finalise",
            Permission::ActAmendFinalised => "act.amend_finalised",
            Permission::ActIssueCopy => "act.issue_copy",
            Permission::RegisterRead => "register.read",
            Permission::RegisterClose => "register.close",
            Permission::AccountingRead => "accounting.read",
            Permission::AccountingWrite => "accounting.write",
            Permission::AccountingClose => "accounting.close",
            Permission::AmlRead => "aml.read",
            Permission::AmlWrite => "aml.write",
            Permission::AmlApprove => "aml.approve",
            Permission::StrExport => "aml.str_export",
            Permission::RulesRead => "rules.read",
            Permission::RulesWrite => "rules.write",
            Permission::RulesApprove => "rules.approve",
            Permission::AuditRead => "audit.read",
            Permission::AuditVerify => "audit.verify",
            Permission::BackupManage => "backup.manage",
            Permission::BackupRestore => "backup.restore",
            Permission::UserManage => "user.manage",
            Permission::SettingsRead => "settings.read",
            Permission::SettingsManage => "settings.manage",
            Permission::ExportCreate => "export.create",
            Permission::ExportApprove => "export.approve",
            Permission::ReportRead => "report.read",
            Permission::InspectionExport => "inspection.export",
            Permission::BreakGlass => "break_glass",
        }
    }

    /// Content permissions: what the IT administrator must never hold (professional secrecy).
    pub fn is_case_content(self) -> bool {
        matches!(
            self,
            Permission::CaseRead
                | Permission::CaseWrite
                | Permission::CaseCreate
                | Permission::CaseDelete
                | Permission::PartyRead
                | Permission::PartyWrite
                | Permission::DocumentRead
                | Permission::DocumentWrite
                | Permission::DocumentDelete
                | Permission::ActRead
                | Permission::ActDraft
                | Permission::ActFinalise
                | Permission::ActAmendFinalised
                | Permission::ActIssueCopy
                | Permission::RegisterRead
                | Permission::AmlRead
                | Permission::AmlWrite
                | Permission::AmlApprove
                | Permission::StrExport
        )
    }
}

const CLERK_PERMS: &[Permission] = &[
    Permission::CaseRead,
    Permission::CaseWrite,
    Permission::CaseCreate,
    Permission::PartyRead,
    Permission::PartyWrite,
    Permission::DocumentRead,
    Permission::DocumentWrite,
    Permission::ActRead,
    Permission::ActDraft,
    Permission::ActIssueCopy,
    Permission::RegisterRead,
    Permission::RulesRead,
    Permission::SettingsRead,
    Permission::ReportRead,
    Permission::ExportCreate,
];

const HEAD_CLERK_PERMS: &[Permission] = &[
    Permission::CaseRead,
    Permission::CaseWrite,
    Permission::CaseCreate,
    Permission::CaseDelete,
    Permission::CaseAclManage,
    Permission::PartyRead,
    Permission::PartyWrite,
    Permission::DocumentRead,
    Permission::DocumentWrite,
    Permission::DocumentDelete,
    Permission::ActRead,
    Permission::ActDraft,
    Permission::ActIssueCopy,
    Permission::RegisterRead,
    Permission::RulesRead,
    Permission::SettingsRead,
    Permission::ReportRead,
    Permission::ExportCreate,
    Permission::AccountingRead,
];

const NOTARY_PERMS: &[Permission] = &[
    // everything on the legal side
    Permission::CaseRead,
    Permission::CaseWrite,
    Permission::CaseCreate,
    Permission::CaseDelete,
    Permission::CaseAclManage,
    Permission::PartyRead,
    Permission::PartyWrite,
    Permission::DocumentRead,
    Permission::DocumentWrite,
    Permission::DocumentDelete,
    Permission::ActRead,
    Permission::ActDraft,
    Permission::ActFinalise,
    Permission::ActAmendFinalised,
    Permission::ActIssueCopy,
    Permission::RegisterRead,
    Permission::RegisterClose,
    Permission::AccountingRead,
    Permission::AccountingWrite,
    Permission::AccountingClose,
    Permission::AmlRead,
    Permission::AmlWrite,
    Permission::AmlApprove,
    Permission::StrExport,
    Permission::RulesRead,
    Permission::RulesWrite,
    Permission::RulesApprove,
    Permission::AuditRead,
    Permission::AuditVerify,
    Permission::BackupRestore,
    Permission::UserManage,
    Permission::SettingsRead,
    Permission::SettingsManage,
    Permission::ExportCreate,
    Permission::ExportApprove,
    Permission::ReportRead,
    Permission::BreakGlass,
];

const ACCOUNTANT_PERMS: &[Permission] = &[
    Permission::AccountingRead,
    Permission::AccountingWrite,
    Permission::CaseRead,
    Permission::DocumentRead,
    Permission::RulesRead,
    Permission::SettingsRead,
    Permission::ReportRead,
    Permission::ExportCreate,
];

const INSPECTOR_PERMS: &[Permission] = &[
    Permission::CaseRead,
    Permission::ActRead,
    Permission::RegisterRead,
    Permission::AccountingRead,
    Permission::AuditRead,
    Permission::AuditVerify,
    Permission::ReportRead,
    Permission::InspectionExport,
    Permission::SettingsRead,
];

const IT_ADMIN_PERMS: &[Permission] = &[
    Permission::UserManage,
    Permission::BackupManage,
    Permission::BackupRestore,
    Permission::SettingsRead,
    Permission::SettingsManage,
    Permission::AuditRead,
    Permission::AuditVerify,
];

const TRAINEE_PERMS: &[Permission] = &[
    Permission::CaseRead,
    Permission::PartyRead,
    Permission::DocumentRead,
    Permission::ActRead,
    Permission::ActDraft,
    Permission::RegisterRead,
    Permission::RulesRead,
    Permission::SettingsRead,
];

/// The matrix. One place to read, one place to change, one place to test.
pub fn grants(role: Role, perm: Permission) -> bool {
    if role == Role::ItAdmin && perm.is_case_content() {
        return false; // absolute: the machine administrator never reads case content
    }
    match role {
        Role::NotaryTitular | Role::NotaryAssociate | Role::NotaryInterim => {
            NOTARY_PERMS.contains(&perm)
        }
        Role::HeadClerk => HEAD_CLERK_PERMS.contains(&perm),
        Role::Clerk => CLERK_PERMS.contains(&perm),
        Role::Trainee => TRAINEE_PERMS.contains(&perm),
        Role::Accountant => ACCOUNTANT_PERMS.contains(&perm),
        Role::Inspector => INSPECTOR_PERMS.contains(&perm),
        Role::ItAdmin => IT_ADMIN_PERMS.contains(&perm),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub user_id: String,
    pub roles: Vec<Role>,
}

impl Principal {
    pub fn new(user_id: impl Into<String>, roles: Vec<Role>) -> Self {
        Principal { user_id: user_id.into(), roles }
    }

    pub fn can(&self, perm: Permission) -> bool {
        self.roles.iter().any(|r| grants(*r, perm))
    }

    pub fn is_notary(&self) -> bool {
        self.roles.iter().any(|r| r.is_notary())
    }

    pub fn require(&self, perm: Permission) -> Result<()> {
        if self.can(perm) {
            Ok(())
        } else {
            Err(CoreError::Denied)
        }
    }

    pub fn require_notary(&self, perm: Permission) -> Result<()> {
        if self.is_notary() && self.can(perm) {
            Ok(())
        } else {
            Err(CoreError::Denied)
        }
    }
}

/// Per-case access-control list. A confidential case is visible only to named users.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CaseAcl {
    pub case_id: String,
    pub confidential: bool,
    pub allowed_user_ids: Vec<String>,
}

impl CaseAcl {
    pub fn allows(&self, user_id: &str) -> bool {
        !self.confidential || self.allowed_user_ids.iter().any(|u| u == user_id)
    }
}

/// Read/write access to one case: permission **and** ACL membership.
pub fn require_case(principal: &Principal, perm: Permission, acl: &CaseAcl) -> Result<()> {
    principal.require(perm)?;
    if acl.allows(&principal.user_id) {
        Ok(())
    } else {
        Err(CoreError::Denied)
    }
}

/// Four-eyes rule: the approver must be a different, authorised person.
pub fn require_four_eyes(
    proposer: &Principal,
    approver: &Principal,
    perm: Permission,
) -> Result<()> {
    if proposer.user_id == approver.user_id {
        return Err(CoreError::Validation(
            "four-eyes: the approver must be a different user".into(),
        ));
    }
    approver.require_notary(perm)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(roles: Vec<Role>) -> Principal {
        Principal::new("u1", roles)
    }

    #[test]
    fn only_notaries_finalise_number_and_sign() {
        assert!(p(vec![Role::NotaryTitular]).can(Permission::ActFinalise));
        for role in [
            Role::HeadClerk,
            Role::Clerk,
            Role::Trainee,
            Role::Accountant,
            Role::Inspector,
            Role::ItAdmin,
        ] {
            assert!(!p(vec![role]).can(Permission::ActFinalise), "{role:?} must not finalise");
        }
    }

    #[test]
    fn it_admin_never_reads_case_content() {
        for perm in [
            Permission::CaseRead,
            Permission::PartyRead,
            Permission::ActRead,
            Permission::DocumentRead,
            Permission::AmlRead,
            Permission::RegisterRead,
        ] {
            assert!(!grants(Role::ItAdmin, perm), "IT admin must not hold {perm:?}");
        }
        // …but it can still run the machine.
        assert!(grants(Role::ItAdmin, Permission::BackupManage));
        assert!(grants(Role::ItAdmin, Permission::UserManage));
    }

    #[test]
    fn inspector_is_read_only() {
        let insp = p(vec![Role::Inspector]);
        assert!(insp.can(Permission::RegisterRead));
        assert!(insp.can(Permission::AccountingRead));
        assert!(!insp.can(Permission::CaseWrite));
        assert!(!insp.can(Permission::ActDraft));
        assert!(!insp.can(Permission::AccountingWrite));
        assert!(!insp.can(Permission::DocumentDelete));
    }

    #[test]
    fn clerk_cannot_touch_accounting_or_aml() {
        let clerk = p(vec![Role::Clerk]);
        assert!(!clerk.can(Permission::AccountingWrite));
        assert!(!clerk.can(Permission::AmlRead));
        assert!(!clerk.can(Permission::StrExport));
    }

    #[test]
    fn confidential_case_hides_from_unnamed_staff() {
        let clerk = p(vec![Role::Clerk]);
        let acl = CaseAcl {
            case_id: "c1".into(),
            confidential: true,
            allowed_user_ids: vec!["u2".into()],
        };
        assert!(matches!(
            require_case(&clerk, Permission::CaseRead, &acl),
            Err(CoreError::Denied)
        ));
        let allowed = Principal::new("u2", vec![Role::Clerk]);
        assert!(require_case(&allowed, Permission::CaseRead, &acl).is_ok());
    }

    #[test]
    fn non_confidential_case_follows_role_only() {
        let clerk = p(vec![Role::Clerk]);
        let acl = CaseAcl { case_id: "c1".into(), confidential: false, allowed_user_ids: vec![] };
        assert!(require_case(&clerk, Permission::CaseRead, &acl).is_ok());
        assert!(require_case(&clerk, Permission::CaseWrite, &acl).is_ok());
        assert!(require_case(&clerk, Permission::ActFinalise, &acl).is_err());
    }

    #[test]
    fn four_eyes_needs_a_different_notary() {
        let proposer = Principal::new("clerk-1", vec![Role::HeadClerk]);
        let notary = Principal::new("notary-1", vec![Role::NotaryTitular]);
        let same = Principal::new("clerk-1", vec![Role::NotaryTitular]);
        assert!(require_four_eyes(&proposer, &notary, Permission::RulesApprove).is_ok());
        assert!(require_four_eyes(&proposer, &same, Permission::RulesApprove).is_err());
    }

    #[test]
    fn approval_permissions_are_notary_only() {
        for role in [Role::HeadClerk, Role::Clerk, Role::Accountant, Role::Inspector] {
            assert!(!p(vec![role]).can(Permission::AmlApprove));
            assert!(!p(vec![role]).can(Permission::AccountingClose));
            assert!(!p(vec![role]).can(Permission::RulesApprove));
        }
    }

    #[test]
    fn accountant_reads_cases_but_cannot_draft_acts() {
        let acc = p(vec![Role::Accountant]);
        assert!(acc.can(Permission::AccountingWrite));
        assert!(acc.can(Permission::CaseRead));
        assert!(!acc.can(Permission::ActDraft));
    }
}
