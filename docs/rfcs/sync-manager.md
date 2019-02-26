# Sync manager

We've identified the need for a "sync manager" (although are yet to identify a good name for it!) This manager will be responsible for managing "global sync state" and coordinating each engine. For the purposes of this, we are including the "clients" engine in this global state.

The primary responsibilities of the engine are:

* Manage the "meta/global" resource. This resources holds the following information:
    * syncID - the global syncID for the entire storage managed by sync.
    * storageVersion - an integer. If this value is greater than a sync client knows about, that client will refuse to sync. While this is rarely changed, it allows us to lock out older sync clients which could be useful.
    * The list of "declined" engines - that is, engines which the user has explicitly declined from syncing. Note that declined engines are global for the account - declining an engine on any device should decline it on every device. While this is something we'd like to consider changing in the future, for now we are sticking with this global declined state.
    * Some metadata about every collection (which may be subtly different from an "engine"). Currently this metadata is only {version, syncID}

    Management of meta/global includes all syncIDs - if there's no global syncID, or an engine has no syncID, it should generate one. As we mention below, engines will be responsible for persisting the syncID and version and taking necessary action when it changes. Thus, engines never need to communicate a new syncID back to the global.

* Query info/collections and pass this information to engines.

* Manage interaction with the token server to obtain a token. As part of this, the manager needs to detect when an engine fails to use the token, generally due to a node reassignment mid sync. In this scenario, the manager will abort any pending syncs and reset itself, starting a new sync flow. Note also that this means the containing application will generally not need to see authentication errors from engines, only from the manager.

* Manage the "clients" collection - we probably can't ignore this any longer, especially for bookmarks (as desktop will send a wipe command on bookmark restore). This involves both communicating with the engine regarding commands targetting it, and accepting commands to be send to other devices. Note however that it's not yet clear where these commands will originate from - eg, consider a bookmark restore - which point in this process will queue the wipe commands for other clients?). Exactly how to manage this (eg, when we can assume the engine has processed the command?) are TBD.

* Perform, or coordinate, the actual sync of the engines - from the containing app's POV, there's a single "sync now" entry-point. Exactly how the manager binds to engines is TBD. For example, a possible implementation is that a callback is registered with the sync manager. It's also likely that we will expose a way to sync a specific engine rather than "sync all".

* Manage the fact that there may not be a 1:1 relationship between collections and sync engines. For example, it may be true in the future that a single engine uses multiple collections. A possible example is when we come to syncing "containers" - the history engine may then use 2 collections as part of a sync.

## What the sync manager will not do

For completeness, this section lists things which the sync manager will not do:

* It is not a sync scheduler. The containing app will be responsible for knowing when to sync (but as above, the scheduler does know *what* to sync.

(anything else?)

# Current implementations and challenges with the Rust components

In most current sync implementations, all engines and management of this global is located in the "sync" code - they are very close to each other and tightly bound. However, the rust components have a different structure which offers some challenges.

* Some apps only care about a subset of the engines - lockbox is one such app and only cares about a single collection/engine.

* Some apps will use a combination of Rust components and "traditional" engines. For example, iOS is moving some of the engines to using Rust components, which other engines are likely to never be ported. We also plan to introduce some rust components into desktop in the same way, meaning desktop will eventually have both rust components and "traditional" engines involved.

* The rust components themselves are designed to be consumed as individual components - the "logins" component doesn't know anything about the "bookmarks" component.

There are a couple of gotchyas in the current implementations too - there's an issue when certain engines don't yet appear in meta/global - see bug 1479929 for all the details.

The tl;dr of the above is that each rust component should be capable of working with different sync managers. In all cases, it is important that meta/global management is done in a holistic and consistent manner.

# Approach for the rust components

In general, we need to pass certain state around, between the "manager" and the engines. We must define this in a way so that the manager can be implemented anywhere - we should not assume the manager is implemented in rust.

We also need to consider the "clients" engine in this state - we will need to communicate commands sent by other devices and commands that we wish to send to other devices.

We will rely on the consuming app to ensure that the sync manager is initialized correctly before syncing any engines.

While we can "trust" each engine, we should try, as much as possible, to isolate engine data from other engines. For example, the keys used to encrypt a collection should only be exposed to that specific engine, and there's no need for one engine to know what info/collections returns for other engines.

# Implementation plan

* The sync manager is responsible for persisting the global syncID and version, considering the state to be "new" if it differs, and failing fatally if the version isn't acceptable. Engines never need to see these values.

* The sync manager is responsible for persisting and populating the "declined" engine list. If an engine is declined, the manager should not ask it to sync. IOW, an engine doesn't really need to know whether it is enabled or not. However, engine should expose a way to be "reset" when the manager notices the declined state for an engine has been changed. This will allow engines to optimize certain things, such as not keeping tomstones etc. As part of this, the sync manager must expose an API so the consuming application can change the disabled engine state.

* The sync manager is responsible for managing the "clients" collection - engines should never see details about that engine. However, engines will need the ability to be told about commands targetting that engine. The sync manager will also need to be able to accept commands to be delivered to other clients - although that needs more thought as the conditions in which a command need to be sent typically aren't managed by the engine itself - eg, when a "bookmark" restore is done.

* Each engine is responsible for persisting engine-specific stuff which needs to be persisted. An obvious thing which needs to be persisted is the syncID and version string for the engine itself, so it can take the necessary action when these change.

* The sync manager will manage keys and pass the relevant key into the engine. Keys for engines should not be exposed to engines other than the key it is for (although currently, in practice, all engines share the same key, but this is an implementation detail.

* The sync manager will pass the relevant part of info/collections to the engine and a token which can be used to access storage. Specific engines never talk to the token server.

# Details
## Structure definitions

While we use rust struct definitions here, it's important to keep in mind that as mentioned above, we'll need to support the manager being written in something other than rust. As the data is relatively small, it may (or may not) be desirable to use JSON as the serialization format.

    // The main structure passed to a sync implementation. This needs to carry
    // all state necessary to sync.
    struct GlobalEngineState {
        // note: no "declined" or "enabled" here - that's invisible to the engine.

        // the result of info/configuration.
        config: GlobalConfig,

        // The token server token.
        storage_token: Vec[u8] (??), // not clear on the type here, but whatever.

        // Info about the collections managed by this engine. In most cases there
        // will be exactly 1.
        collections: HashMap<String, CollectionState>
    }

    // The state for a collection. Most engines will manage a single collection, but
    // there may be many.
    struct CollectionState {
        // The info/collections response for this collection.
        collection_info: CollectionInfo,

        // The current syncID for the engine, as read from meta/global. May be None.
        // Engines are responsible for persisting this and taking action when it differs.
        sync_id: Option<SyncGuid>,

        // The keys to be used for this collection. This may be the default key (ie,
        // used for all collections) or one specific to the collection, but the
        // engine doesn't need to know that.
        keys: KeyBundle,

        // Commands the engine should act upon before syncing.
        commands: Vec<Command>
    }

     struct Command {
        ...
    }

## APIs
## Sync Manager

    // Register an engine.
    fn register_engine(name: String, collections: Vec<String>, sync_callback: Fn<...>)

    // Get the list of engines which are declined. There's a bit of confusion here
    // between an "engine" and a "collection" - eg, assuming history ends up using
    // 2 collections, the UI will want 1 entry.
    // Maybe we define this as a "collection" and the app should ignore collections
    // it doesn't understand?
    // Will return None if we haven't yet done a first sync so don't know. Will
    // return an empty vec if we have synced but no engines are declined.
    fn get_declined() -> Option<Vec<String>>

    // Ditto here - a bit of confusion between "collection" and "engine" here.
    fn enable_engine(String, bool)

    // TODO: let's think a little more about how desktop uses a single "addons"
    // pref which controls addons *and* storage.sync, and how a single "Forms"
    // pref controls 2 discrete engines.

    // Do a sync now. There's a special error code to indicate that something
    // seems to have gone wrong with auth, which is surfaced to the app, so
    // it can call on FxA to do magic.
    fn sync_now(engines: Option<Vec<String>>) -> Result<()>

    // Stop syncing ASAP. Called as the app is shutting down, and possibly
    // in other circumstances, such as when wifi is lost but the user has
    // opted in to syncing only on wifi
    // TBD - does this make sense? If so, how long is the app expected to wait?
    fn stop()

    // others?

## Engines

    // Perform the sync of the engine. When asked to sync, the manager will
    // create a GlobalEngineState specific to the engine and pass it in.
    fn sync(state: GlobalEngineState) -> ???

#Notes

Just random notes by markh - this should be either deleted or incorporated into the above

meta/global has:
syncID: 
storageVersion: always 5
declined: list of declined engines
engines: list of {version, syncID} meta about each non-declined engine
Desktop:

* First sync for a session fetches meta/global.
* 401 means "node reassignment" so early exit.
* 404 means first sync on a new node - must be uploaded.

then uploads partial meta/global - no engines are there yet. It uploads engines as they are synced the first time.
