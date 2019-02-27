# Sync manager

We've identified the need for a "sync manager" (although are yet to identify a
good name for it!) This manager will be responsible for managing "global sync
state" and coordinating each engine. For the purposes of this, we are
including the "clients" engine in this global state.

The primary responsibilities of the engine are:

* Manage the "meta/global" resource. This resources holds the following information:

    * syncID - the global syncID for the entire storage managed by sync.

    * storageVersion - an integer. If this value is greater than a sync client
      knows about, that client will refuse to sync. While this is rarely
      changed, it allows us to lock out older sync clients which could be
      useful.

    * The list of "declined" engines - that is, engines which the user has
      explicitly declined from syncing. Note that declined engines are global
      for the account - declining an engine on any device should decline it on
      every device. While this is something we'd like to consider changing in
      the future, for now we are sticking with this global declined state.

    * Some metadata about every collection (which may be subtly different from
      an "engine"). Currently this metadata is only {version, syncID}

    Management of meta/global includes all syncIDs - if there's no global
    syncID, or a collection has no syncID, it should generate one. As we mention
    below, engines will be responsible for persisting the syncID and version
    and taking necessary action when it changes. Thus, engines never need to
    communicate a new syncID back to the global.

* Query info/collections and pass this information to engines.

* Manage interaction with the token server to obtain a token. As part of this,
  the manager needs to detect when an engine fails to use the token, either
  due to token expiry, or due to a node reassignment mid sync. The manager will
  attempt to fetch a new token, and if successful, re-start the sync of the
  failing engine. If that succeeds but the node has changed, the manager will
  perform a full reset and restart a sync flow for all engines. If obtaining
  the new token fails due to auth issues, it will signal to the containing
  app that it should take some auth action before it can continue.
  (Note that the manager can help reduce instances of token expiry by ensuring
  the token is valid for some reasonable time before handing it to an engine)

* Manage the "clients" collection - we probably can't ignore this any longer,
  especially for bookmarks (as desktop will send a wipe command on bookmark
  restore). This involves both communicating with the engine regarding
  commands targetting it, and accepting commands to be send to other devices.
  Note however that outgoing commands are likely to not originate from a sync,
  but instead from other actions, such as "restore bookmarks". While the
  store *may* be involved in that (and thus able to queue the outgoing command),
  we should not assume that's always the case. Exactly how to manage this (eg,
  when we can assume the engine has processed the command?) are TBD.

* Perform, or coordinate, the actual sync of the engines - from the containing
  app's POV, there's a single "sync now" entry-point. Exactly how the manager
  binds to engines is TBD. It's also likely that we will expose a way to sync
  a specific engine rather than "sync all".

* Manage the fact that there may not be a 1:1 relationship between collections
  and sync engines. For example, it may be true in the future that when we come to
  syncing "containers", the history engine might require 2 collections to
  sync. Note that this is subtly different from engines which happen to be
  closely related - while history and bookmarks, or addresses and credit-cards
  are related, each can be synced independently so are *not* examples of
  engines which would leverage this.

  XXX - do we actually need this now? Should we ignore it and wait for
  a concrete use-case? If we support this in the future, we can probably still
  have sugar so that the common case of a 1:1 relationship doesn't need to
  deal with this.

* It will manage the collection of telemetry from the engines and pass this
  info back to the app for submission.

## What the sync manager will not do

For completeness, this section lists things which the sync manager will not do:

* It is not a sync scheduler. The containing app will be responsible for
  knowing when to sync (but as above, the scheduler does know *what* to sync.

* It will not perform authentication - it's role is to signal to the app when
  an auth problem exists and the app is expected to resolve that.

* It will not directly talk to the token server, although it *will* coordinate
  use of a token-server client to manage that communication.

* It will not submit telemetry.

* It does not track any changes or validate any collections - these remain the
  responsibility of the engines.


(anything else?)

# Current implementations and challenges with the Rust components

In most current sync implementations, all engines and management of this
global is located in the "sync" code - they are very close to each other and
tightly bound. However, the rust components have a different structure which
offers some challenges.

* Some apps only care about a subset of the engines - lockbox is one such app
  and only cares about a single collection/engine.

* Some apps will use a combination of Rust components and "traditional"
  engines. For example, iOS is moving some of the engines to using Rust
  components, which other engines are likely to never be ported. We also plan
  to introduce some rust components into desktop in the same way, meaning
  desktop will eventually have both rust components and "traditional" engines
  involved.

* The rust components themselves are designed to be consumed as individual
  components - the "logins" component doesn't know anything about the
  "bookmarks" component.

There are a couple of gotchyas in the current implementations too - there's an
issue when certain engines don't yet appear in meta/global - see bug 1479929
for all the details.

The tl;dr of the above is that each rust component should be capable of
working with different sync managers. In all cases, it is important that
meta/global management is done in a holistic and consistent manner.

# Approach for the rust components

In general, we need to pass certain state around, between the "manager" and
the engines. We must define this in a way so that the manager can be
implemented anywhere - we should not assume the manager is implemented in
rust.

The individual rust components will not manage the "clients" engine - that will
be done by the manager - it need to communicate commands sent by other devices
and commands need to be sent to other devices.

We will rely on the consuming app to ensure that the sync manager is
initialized correctly before syncing any engines.

While we can "trust" each engine, we should try, as much as possible, to
isolate engine data from other engines. For example, the keys used to encrypt
a collection should only be exposed to that specific engine, and there's no
need for one engine to know what info/collections returns for other engines.

# Implementation plan

* The sync manager is responsible for persisting the global syncID and
  version, considering the state to be "new" if it differs, and failing
  fatally if the version isn't acceptable. Engines never need to see these
  values.

* The sync manager is responsible for managing info/collections, downloading
  it and uploading it when it needs to be changed. While it is an implementation
  detail how this is done exactly, important requirements are that it:

  * Must use if-modified-since to ensure that another client hasn't raced to
    update it.

  * Ensure that entries for collections which we don't understand are managed
    correctly (ie, not discarded or otherwise changed)

* The sync manager is responsible for persisting and populating the "declined"
  engine list. If an engine is declined, the manager should not ask it to
  sync. IOW, an engine doesn't really need to know whether it is enabled or
  not. However, engines should expose a way to be "reset" when the manager
  notices the declined state for an engine has been changed. This will allow
  engines to optimize certain things, such as not keeping tomstones etc. As
  part of this, the sync manager must expose an API so the consuming
  application can change the disabled engine state.

* The sync manager is responsible for managing the "clients" collection -
  engines should never see details about that engine. However, engines will
  need the ability to be told about commands targetting that engine. The sync
  manager will also need to be able to accept commands to be delivered to
  other clients - although that needs more thought as the conditions in which
  a command need to be sent typically aren't managed by the engine itself -
  eg, when a "bookmark" restore is done.

* Each engine is responsible for persisting engine-specific stuff which needs
  to be persisted. An obvious thing which needs to be persisted is the syncID
  and version string for the engine itself, so it can take the necessary
  action when these change.

* The sync manager will call a special "prepare" function on the engine,
  passing the current global version and the engine's current syncID and
  version. This function will return an enum of actions the manager should
  take. Initial supported actions will include only "can't sync due to version",
  but we forsee a requirement in the future which indicates "please abandon
  this syncID and generate a new one" to support the use-case described in
  [this bug](https://bugzilla.mozilla.org/show_bug.cgi?id=1199077#c23).
  Note that this function could fail (eg, storage might be corrupt), in which
  case the engine will not be asked to sync.

* The sync manager will manage keys and pass the relevant key into the engine.
  Keys for engines should not be exposed to engines other than the key it is
  for (although currently, in practice, all engines share the same key, but
  this is an implementation detail.)

* The sync manager will pass the relevant part of info/collections to the
  engine and a token which can be used to access storage. Specific engines
  never talk to the token server.

## Rust implementation plan

We will implement a sync manager in Rust. In the first instance, the Rust
implemented manager will only support Rust implemented engines - there are
no current requirements for this implementation to support "external" engines.

Further, this Rust implementation will only support engines implemented in the
same library as the manager. This should ease the registration and calling of
engines by the rust code and avoid requiring the FFI to pass object or
function references around. It also means that there remains a single function
in our library to perform the sync of whatever engines are in the library.

## External implementation plans

We have identified that iOS will, at least in the short term, want the
sync manager to be implemented in Swift. This will be responsible for
syncing both the Swift and Rust implemented engines.

At some point in the future, Desktop may do the same - we will have both
Rust and JS implemented engines which need to be coordinated. We ignore this
requirement for now.

This approach still has a fairly easy time coordinating with the Rust
implemented engines - the FFI will need to expose the relevant sync
entry-points to be called by Swift, but the Swift code can hard-code the
Rust engines it has and make explicit calls to these entry-points.

This Swift code will need to create the structures identified below, but this
shouldn't be too much of a burden as it already has the information necessary
to do so (ie, it already has info/collections etc)

TODO: dig into the Swift code and make sure this is sane.

# Details
## Structure definitions

While we use rust struct definitions here, it's important to keep in mind that
as mentioned above, we'll need to support the manager being written in
something other than rust. The definitions below are designed to be easy to
serialize and used across the FFI - probably using protobufs, although JSON
would also be an option if there was a good reason to prefer that.

```rust
// The main structure passed to a sync implementation. This needs to carry
// all state necessary to sync.
struct GlobalEngineState {
    // note: no "declined" or "enabled" here - that's invisible to the engine.

    // the result of info/configuration.
    config: GlobalConfig,

    // The token server token, used for authenticating with the storage servers.
    storage_token: Vec<u8>, // (??) not clear on the type here, but whatever.

    // Info about the collections managed by this engine. In most cases there
    // will be exactly 1.
    collections: HashMap<String, CollectionState>
}

// The state for a collection. Most engines will manage a single collection, but
// there may be many.
struct CollectionState {
    // The info/collections response for this collection.
    collection_info: CollectionInfo,

    // The current syncID for the engine, as read from meta/global. Will never
    // be None as the sync manager is responsbile for generating an ID if
    // it doesn't exist.
    // Engines are responsible for persisting this and taking action when it
    // differs from last time.
    sync_id: SyncGuid,

    // The current version for the engine, as read from meta/global.
    // Not clear that "u32" is the best option here - I guess it's OK so
    // long as we treat an invalid u32 representation sanely.
    // Note that persisting this may not be required if there's only one
    // version supported, as the value can be compared against a hard-coded
    // value in the engine.
    version: u32,

    // The keys to be used for this collection. This may be the default key (ie,
    // used for all collections) or one specific to the collection, but the
    // engine doesn't need to know that.
    keys: KeyBundle,

    // Commands the engine should act upon before syncing.
    commands: Vec<Command>
}

// Used to support commands. Will either be a strongly typed enum, or a
// weakly typed thing (such as {command: String, args: Vec<String>})
// TBD.
struct Command {
    ...
}
```

## APIs
## Sync Manager

```rust
// Get the list of engines which are declined. There's a bit of confusion here
// between an "engine" and a "collection" - eg, assuming history ends up using
// 2 collections, the UI will want 1 entry.
// Maybe we define this as a "collection" and the app should ignore collections
// it doesn't understand?
// Will return None if we haven't yet done a first sync so don't know. Will
// return an empty vec if we have synced but no engines are declined.
fn get_declined() -> Option<Vec<String>>;

// Ditto here - a bit of confusion between "collection" and "engine" here.
fn enable_engine(String, bool) -> Result<()>;

// TODO: let's think a little more about how desktop uses a single "addons"
// pref which controls addons *and* storage.sync, and how a single "Forms"
// pref controls 2 discrete engines.

// Do a sync now. There's a special error code to indicate that something
// seems to have gone wrong with auth, which is surfaced to the app, so
// it can call on FxA to do magic.
fn sync_now(engines: Option<Vec<String>>) -> Result<()>;

// Stop syncing ASAP. This might be called as the app is shutting down or when
// the app has lost wifi, for example.
fn stop();

// others?
```

## Engines

```rust
#[repr(u8)]
pub enum PrepareAction {
    // No special action is required.
    NoAction = 0,
    // This engine can not sync because the version already on the server is
    // greater than this engine supports.
    VersionLockout = 1,
    // The sync engine is attempting to recover from an unusual state and the
    // existing syncID should be discarded.
    NewSyncId = 2,
}

// Prepare for a sync. Passed whatever values are in info/collections
// before the sync manager has generated new GUIDs or versions (ie, on
// the very first sync to a new storage node, we'd expect all to be None).
// Result is the action the manager should take before calling the sync
// method.
fn prepare(global_version: Option<u32>, engine_syncid: Option<SyncGuid>, engine_version: Option<u32>) -> Result<PrepareAction>;

// Perform the sync of the engine. When asked to sync, the manager will
// create a GlobalEngineState specific to the engine and pass it in.
fn sync(state: GlobalEngineState) -> Result<SomeTelemetryObject>;
```

### Notes

Just random notes by markh - this should be either deleted or incorporated
into the above

meta/global has:
syncID: 
storageVersion: always 5
declined: list of declined engines
engines: list of {version, syncID} meta about each non-declined engine
Desktop:

* First sync for a session fetches meta/global.
* 401 means "node reassignment" so early exit.
* 404 means first sync on a new node - must be uploaded.

then uploads partial meta/global - no engines are there yet. It uploads
engines as they are synced the first time.
