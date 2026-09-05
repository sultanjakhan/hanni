package com.sultanjakhan.hanni

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Test

class HealthConnectPagingTest {
    @Test fun consumesAllPagesAndStopsOnEmptyTerminalToken() = runBlocking {
        val calls = mutableListOf<String?>()
        val records = collectHealthPages<Int> { token ->
            calls.add(token)
            check(calls.size <= 2) { "Reader repeated terminal page" }
            if (token == null) Pair(listOf(1,2), "next") else Pair(listOf(3), "")
        }
        assertEquals(listOf(1,2,3), records)
        assertEquals(listOf(null,"next"), calls)
    }

    @Test fun emptyFirstPageIsTerminalWithoutRepeatedRead() = runBlocking {
        var calls = 0
        val records = collectHealthPages<Int> {
            calls++
            check(calls == 1) { "Reader repeated empty terminal page" }
            Pair(emptyList(), "")
        }
        assertEquals(emptyList<Int>(), records)
        assertEquals(1, calls)
    }

    @Test fun nullTerminalTokenRetainsExistingProviderBehavior() = runBlocking {
        var calls = 0
        val records = collectHealthPages<Int> {
            calls++
            check(calls == 1)
            Pair(listOf(1), null)
        }
        assertEquals(listOf(1), records)
        assertEquals(1, calls)
    }
}
