/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

package mozilla.appservices.support

import mozilla.components.concept.fetch.Client

// We pass `null` in during tests
fun initMegazord(name: String, client: Lazy<Client>?) {
    System.setProperty("mozilla.appservices.megazord", name)
    client?.let {
        val httpConfigInitClass = Class.forName("mozilla.appservices.httpconfig.RustHttpInit")
        val initFunc = httpConfigInitClass.getDeclaredMethod("init", Lazy::class.java)
        initFunc.invoke(httpConfigInitClass, it)
    }
    // TODO: Can we move android-component's RustLog here now?
}
