/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

package mozilla.appservices

import mozilla.components.concept.fetch.Client

/** The 'full' megazord (used when a specific megazord doesn't override it) */
object Megazord {
    const val NAME = "megazord"
    fun init(client: Lazy<Client>) {
        mozilla.appservices.support.initMegazord(NAME, client)
    }

    // We don't really need to care about this outside of testing, it's reasonable to expect
    // we'll only be initialized once (Probably?)
    private var initialized = false
    // Note: This should only be on the 'full' megazord, and not on the
    // other megazords (probably?)
    @Synchronized
    fun initForTesting(client: Lazy<Client>? = null) {
        if (!initialized) {
            initialized = true
            mozilla.appservices.support.initMegazord(NAME, client)
        }
    }
}
