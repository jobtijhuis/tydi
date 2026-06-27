use std::convert::TryInto;

use super::{Assign, AssignDeclaration, Assignment, AssignmentKind, ObjectAssignment};
use crate::{stdlib::common::architecture::declaration::ObjectDeclaration, Error, Result};

impl Assign for ObjectDeclaration {
    fn assign(&self, assignment: &(impl Into<Assignment> + Clone)) -> Result<AssignDeclaration> {
        let true_assignment = assignment.clone().into();
        self.typ().can_assign(&true_assignment)?;
        Ok(AssignDeclaration::new(self.clone(), true_assignment))
    }
}

impl ObjectDeclaration {
    /// Assign this object from the concatenation (VHDL `&`) of multiple objects,
    /// listed most-significant first.
    pub fn assign_concat(
        &self,
        objects: &[impl Into<ObjectAssignment> + Clone],
    ) -> Result<AssignDeclaration> {
        let concatenation = AssignmentKind::Concatenation(
            objects.iter().map(|object| object.clone().into()).collect(),
        );
        self.assign(&Assignment::from(concatenation))
    }
}

impl<T> Assign for T
where
    T: TryInto<ObjectDeclaration, Error = Error> + Clone,
{
    fn assign(&self, assignment: &(impl Into<Assignment> + Clone)) -> Result<AssignDeclaration> {
        let decl = self.clone().try_into()?;
        decl.assign(assignment)
    }
}
