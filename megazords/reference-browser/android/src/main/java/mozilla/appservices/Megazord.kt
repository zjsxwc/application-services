/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

package mozilla.appservices

import mozilla.components.concept.fetch.Client

/** The megazord for the reference browser */
object Megazord {
    const val NAME = "reference_browser"
    fun init(client: Lazy<Client>) {
        mozilla.appservices.support.initMegazord(NAME, client)
    }

}
