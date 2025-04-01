// It's pointless to run miri on UI tests
#[cfg_attr(miri, ignore)]
mod snapshot_testing;
