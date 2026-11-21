# FIN03 unstage native recovery and UI

Adds user-visible single-staged-file native review, one-use OS consent,
original receipt verification and journal inspection. On an interrupted
index write, a **distinct native consent** permits re-reading the
same repository, exact HEAD, index and staged entries, then appending
one of two independently verified terminal journal states:
- unchanged index: cancelled without publication
- index matches exact HEAD with no competing staged change: verified

No replay, stash mutation, rollback or branch operation is performed.
An ambiguous index stays quarantined until manual repair. Does not
constitute installed Windows/Linux approval acceptance.
