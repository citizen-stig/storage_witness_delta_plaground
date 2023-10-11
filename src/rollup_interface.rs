pub trait STF {
    type Witness;
    type BlobTransaction;

    type SnapshotRef;
    type ChangeSet;


    fn apply_slot<'a, I>(
        &mut self,
        base: Self::SnapshotRef,
        blobs: I,
    ) -> (Self::Witness, Self::ChangeSet)
        where
            I: IntoIterator<Item=Self::BlobTransaction>;
}