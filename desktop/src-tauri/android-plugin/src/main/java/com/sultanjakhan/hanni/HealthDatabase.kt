package com.sultanjakhan.hanni

import android.content.ContentValues
import android.database.Cursor
import android.database.MatrixCursor
import android.util.Base64
import androidx.annotation.Keep
import org.json.JSONArray
import org.json.JSONObject
import java.io.Closeable

/** Internal SQL surface. Production implementations must use the shared native SQLite engine. */
internal interface HealthDatabase : Closeable {
    fun rawQuery(sql: String, args: Array<out String>?): Cursor
    fun execSQL(sql: String, args: Array<out Any?> = emptyArray())
    fun insertOrThrow(table: String, nullColumnHack: String?, values: ContentValues): Long
    fun insertWithOnConflict(table: String, nullColumnHack: String?, values: ContentValues, conflict: Int): Long
    fun update(table: String, values: ContentValues, where: String?, args: Array<out String>?): Int
    fun delete(table: String, where: String?, args: Array<out String>?): Int
    fun compileStatement(sql: String): HealthStatement
    fun beginTransaction()
    fun setTransactionSuccessful()
    fun endTransaction()
    fun inTransaction(): Boolean
    companion object {
        const val CONFLICT_NONE = 0
        const val CONFLICT_IGNORE = 4
        const val CONFLICT_REPLACE = 5
    }
}

internal interface HealthStatement : Closeable {
    fun bindLong(index: Int, value: Long)
    fun executeUpdateDelete(): Int
}

internal class HealthDatabaseException(val code: String) : IllegalStateException(code)

/** Only values are transported: numeric payloads are strings to preserve every Int64 bit. */
internal object HealthDatabaseWire {
    const val LIMIT = 16 * 1024 * 1024
    fun text(value: String): String {
        var index = 0
        while (index < value.length) {
            val char = value[index++]
            if (char.isHighSurrogate()) {
                require(index < value.length && value[index++].isLowSurrogate()) { "native_db_arguments" }
            } else require(!char.isLowSurrogate()) { "native_db_arguments" }
        }
        return value
    }
    fun encode(value: Any?): JSONObject {
        val (kind, content) = when (value) {
            null -> "n" to null
            is Byte, is Short, is Int, is Long -> "i" to (value as Number).toLong().toString()
            is Boolean -> "i" to if (value) "1" else "0"
            is Float, is Double -> {
                val number = (value as Number).toDouble()
                require(number.isFinite()) { "native_db_arguments" }
                "f" to number.toString()
            }
            is String -> "s" to text(value)
            is ByteArray -> "b" to Base64.encodeToString(value, Base64.NO_WRAP)
            else -> throw HealthDatabaseException("native_db_arguments")
        }
        return JSONObject().put("t", kind).apply { if (content != null) put("v", content) }
    }
    fun decode(cell: JSONObject): Any? {
        val kind = cell.get("t") as? String ?: throw HealthDatabaseException("native_db_value")
        require(cell.length() == if (kind == "n") 1 else 2) { "native_db_value" }
        if (kind == "n") return null
        val value = cell.get("v") as? String ?: throw HealthDatabaseException("native_db_value")
        return when (kind) {
            "i" -> value.toLong().also { require(it.toString() == value) { "native_db_value" } }
            "f" -> value.toDouble().also { require(it.isFinite()) { "native_db_value" } }
            "s" -> text(value)
            "b" -> Base64.decode(value, Base64.NO_WRAP).also {
                require(Base64.encodeToString(it, Base64.NO_WRAP) == value) { "native_db_value" }
            }
            else -> throw HealthDatabaseException("native_db_value")
        }
    }
    fun cursor(value: JSONObject): Cursor {
        val columns = value.getJSONArray("columns")
        val names = Array(columns.length()) { text(columns.get(it) as? String ?: throw HealthDatabaseException("native_db_value")) }
        val rows = value.getJSONArray("rows")
        require(rows.length() <= 10_000) { "native_db_limit" }
        val cursor = MatrixCursor(names, rows.length())
        try {
            for (i in 0 until rows.length()) {
                val row = rows.getJSONArray(i)
                require(row.length() == names.size) { "native_db_value" }
                cursor.addRow(Array<Any?>(names.size) { decode(row.getJSONObject(it)) })
            }
            return cursor
        } catch (error: Exception) { cursor.close(); throw error }
    }
}

/** Owns one native connection; closing always releases its handle and rolls back pending work. */
internal class NativeHealthDatabase private constructor(private var handle: String) : HealthDatabase {
    companion object {
        fun openExisting(path: String): NativeHealthDatabase {
            val result = invoke(JSONObject().put("op", "open").put("path", HealthDatabaseWire.text(path))) as? JSONObject
                ?: throw HealthDatabaseException("native_db_value")
            val handle = result.get("handle") as? String ?: throw HealthDatabaseException("native_db_value")
            require(handle.toLong() > 0 && handle.toLong().toString() == handle) { "native_db_value" }
            return NativeHealthDatabase(handle)
        }
        private fun invoke(request: JSONObject): Any? {
            val encoded = request.toString()
            require(encoded.toByteArray(Charsets.UTF_8).size <= HealthDatabaseWire.LIMIT) { "native_db_limit" }
            val raw = HealthDatabaseNative.nativeInvoke(encoded)
            require(raw.toByteArray(Charsets.UTF_8).size <= HealthDatabaseWire.LIMIT) { "native_db_limit" }
            val reply = JSONObject(raw)
            if (reply.opt("ok") != true) {
                val code = reply.optString("error")
                throw HealthDatabaseException(if (code in setOf("native_db_arguments", "native_db_closed",
                    "native_db_failed", "native_db_limit", "native_db_transaction", "native_db_value",
                    "native_db_not_ready")) code else "native_db_failed")
            }
            return reply.get("result").takeUnless { it === JSONObject.NULL }
        }
        private fun identifier(value: String): String {
            require(Regex("[A-Za-z_][A-Za-z0-9_]*").matches(value)) { "native_db_arguments" }
            return "\"$value\""
        }
    }

    @Synchronized private fun call(op: String, sql: String? = null, args: Array<out Any?> = emptyArray()): Any? {
        check(handle.isNotEmpty()) { "native_db_closed" }
        return invoke(JSONObject().put("op", op).put("handle", handle).apply {
            if (sql != null) {
                put("sql", HealthDatabaseWire.text(sql))
                put("args", JSONArray().apply { for (value in args) put(HealthDatabaseWire.encode(value)) })
            }
        })
    }
    private fun execute(sql: String, args: Array<out Any?> = emptyArray()): JSONObject =
        call("execute", sql, args) as? JSONObject ?: throw HealthDatabaseException("native_db_value")
    private fun changes(result: JSONObject): Int = result.getString("changes").toInt().also { require(it >= 0) }

    override fun rawQuery(sql: String, args: Array<out String>?): Cursor = HealthDatabaseWire.cursor(
        call("query", sql, args ?: emptyArray()) as? JSONObject ?: throw HealthDatabaseException("native_db_value"))
    override fun execSQL(sql: String, args: Array<out Any?>) { execute(sql, args) }
    override fun insertOrThrow(table: String, nullColumnHack: String?, values: ContentValues): Long =
        insertWithOnConflict(table, nullColumnHack, values, HealthDatabase.CONFLICT_NONE)
    override fun insertWithOnConflict(table: String, nullColumnHack: String?, values: ContentValues, conflict: Int): Long {
        require(nullColumnHack == null && values.size() > 0) { "native_db_arguments" }
        val modifier = when (conflict) {
            HealthDatabase.CONFLICT_NONE -> ""
            HealthDatabase.CONFLICT_IGNORE -> " OR IGNORE"
            HealthDatabase.CONFLICT_REPLACE -> " OR REPLACE"
            else -> throw HealthDatabaseException("native_db_arguments")
        }
        val keys = values.keySet().toList()
        val result = execute("INSERT$modifier INTO ${identifier(table)} (${keys.joinToString(",") { identifier(it) }}) VALUES (${keys.joinToString(",") { "?" }})",
            keys.map { values.get(it) }.toTypedArray())
        return if (changes(result) == 0) -1 else result.getString("row_id").toLong()
    }
    override fun update(table: String, values: ContentValues, where: String?, args: Array<out String>?): Int {
        require(values.size() > 0) { "native_db_arguments" }
        val keys = values.keySet().toList()
        return changes(execute("UPDATE ${identifier(table)} SET ${keys.joinToString(",") { "${identifier(it)}=?" }}${where?.let { " WHERE $it" } ?: ""}",
            (keys.map { values.get(it) } + (args?.toList() ?: emptyList())).toTypedArray()))
    }
    override fun delete(table: String, where: String?, args: Array<out String>?): Int = changes(
        execute("DELETE FROM ${identifier(table)}${where?.let { " WHERE $it" } ?: ""}", args ?: emptyArray()))
    override fun compileStatement(sql: String): HealthStatement = object : HealthStatement {
        private var closed = false
        private val bindings = sortedMapOf<Int, Long>()
        override fun bindLong(index: Int, value: Long) {
            check(!closed); require(index in 1..1000); bindings[index] = value
        }
        override fun executeUpdateDelete(): Int {
            check(!closed)
            val maximum = bindings.keys.lastOrNull() ?: 0
            require(bindings.size == maximum) { "native_db_arguments" }
            return changes(execute(sql, (1..maximum).map { bindings.getValue(it) }.toTypedArray()))
        }
        override fun close() { closed = true; bindings.clear() }
    }
    override fun beginTransaction() { call("begin") }
    override fun setTransactionSuccessful() { call("successful") }
    override fun endTransaction() { call("end") }
    override fun inTransaction(): Boolean = call("in_transaction") as? Boolean ?: throw HealthDatabaseException("native_db_value")
    @Synchronized override fun close() {
        if (handle.isEmpty()) return
        val prior = handle
        handle = ""
        invoke(JSONObject().put("op", "close").put("handle", prior))
    }
}

@Keep
internal object HealthDatabaseNative {
    init { System.loadLibrary("hanni_lib") }
    @Keep @JvmStatic external fun nativeInvoke(request: String): String
}
