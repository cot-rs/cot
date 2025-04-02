// It's pointless to run miri on UI tests
#[cfg(not(miri))]
mod snapshot_testing;
