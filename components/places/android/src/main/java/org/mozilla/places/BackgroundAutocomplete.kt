/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

package org.mozilla.places

import kotlinx.coroutines.CompletableDeferred
import java.util.concurrent.CancellationException
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.atomic.AtomicLong

private class AutocompleteRequest(
        val id: Long,
        val query: String,
        val deferred: CompletableDeferred<List<SearchResult>>
)

class BackgroundAutocomplete internal constructor(db: PlacesConnection): AutoCloseable {
    private val lastId: AtomicLong = AtomicLong(0)
    private val activeId: AtomicLong = AtomicLong(-1)
    private val requests: LinkedBlockingQueue<AutocompleteRequest> = LinkedBlockingQueue()

    private val interrupt: InterruptHandle = db.getInterruptHandle()

    // TODO: Do we want to keep this around forever?
    private val bgThread: AutocompleteThread = AutocompleteThread(
            lastId, activeId, requests, db)

    // We don't want this connection shared with anybody.
    constructor(dbPath: String): this(PlacesConnection(dbPath))

    suspend fun getResults(query: String): List<SearchResult> {
        val id = this.lastId.incrementAndGet()
        val deferred = CompletableDeferred<List<SearchResult>>()
        interrupt.interrupt();
        requests.add(AutocompleteRequest(id, query, deferred))
        try {
            return deferred.await()
        } catch (e: CancellationException) {
            // Only interrupt it if it's still working on
            // the current query.
            if (activeId.get() == id) {
                interrupt.interrupt()
            }
            // Rethrow this I guess?
            throw e
        }
    }

    fun stop() {
        requests.clear()
        lastId.incrementAndGet()
        interrupt.interrupt()
    }

    override fun close() {
        interrupt.close()
        bgThread.close()
        // Does this make sense?
        if (!bgThread.isInterrupted) {
            bgThread.interrupt()
        }
    }
}

// This should probably be some kotlin Job or something and not a Thread, IDK.
private class AutocompleteThread(
        val lastId: AtomicLong,
        val activeId: AtomicLong,
        val requests: LinkedBlockingQueue<AutocompleteRequest>,
        val db: PlacesConnection
) : Thread(), AutoCloseable {

    override fun close() {
        db.close()
    }

    override fun run() {
        while (true) {
            activeId.set(-1)
            val request = this.requests.poll()
            if (request.id != lastId.get() || request.deferred.isCancelled) {
                continue
            }
            activeId.set(request.id)
            val result = try {
                db.queryAutocomplete(request.query, 10)
            } catch (e: PlacesException) {
                if (shouldFulfill(request)) {
                    request.deferred.completeExceptionally(e)
                }
                // Otherwise not only does nobody care that we failed,
                // it's probably a interruption anyway.
                continue
            }
            if (shouldFulfill(request)) {
                request.deferred.complete(result)
            }
        }
    }

    fun shouldFulfill(req: AutocompleteRequest): Boolean {
        return lastId.get() == req.id && !req.deferred.isCancelled
    }
}
