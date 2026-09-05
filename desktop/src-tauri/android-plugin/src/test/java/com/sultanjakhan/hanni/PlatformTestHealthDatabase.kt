package com.sultanjakhan.hanni

import android.content.ContentValues
import android.database.Cursor
import android.database.sqlite.SQLiteDatabase
import java.io.File

/** Test-only adapter: keeps the established SQLite semantic regressions independent of JNI. */
internal class PlatformTestHealthDatabase private constructor(private val db: SQLiteDatabase) : HealthDatabase {
    companion object {
        const val OPEN_READWRITE = SQLiteDatabase.OPEN_READWRITE
        fun create(factory: SQLiteDatabase.CursorFactory?) = PlatformTestHealthDatabase(SQLiteDatabase.create(factory))
        fun openOrCreateDatabase(file: File, factory: SQLiteDatabase.CursorFactory?) =
            PlatformTestHealthDatabase(SQLiteDatabase.openOrCreateDatabase(file, factory))
        fun openDatabase(path: String, factory: SQLiteDatabase.CursorFactory?, flags: Int) =
            PlatformTestHealthDatabase(SQLiteDatabase.openDatabase(path, factory, flags))
        fun deleteDatabase(file: File) = SQLiteDatabase.deleteDatabase(file)
    }
    override fun rawQuery(sql: String, args: Array<out String>?): Cursor = db.rawQuery(sql, args)
    override fun execSQL(sql: String, args: Array<out Any?>) {
        if (args.isEmpty()) db.execSQL(sql) else db.execSQL(sql, args)
    }
    override fun insertOrThrow(table: String, nullColumnHack: String?, values: ContentValues) = db.insertOrThrow(table, nullColumnHack, values)
    override fun insertWithOnConflict(table: String, nullColumnHack: String?, values: ContentValues, conflict: Int) = db.insertWithOnConflict(table, nullColumnHack, values, conflict)
    override fun update(table: String, values: ContentValues, where: String?, args: Array<out String>?) = db.update(table, values, where, args)
    override fun delete(table: String, where: String?, args: Array<out String>?) = db.delete(table, where, args)
    override fun compileStatement(sql: String): HealthStatement {
        val statement = db.compileStatement(sql)
        return object : HealthStatement {
            override fun bindLong(index: Int, value: Long) = statement.bindLong(index, value)
            override fun executeUpdateDelete() = statement.executeUpdateDelete()
            override fun close() = statement.close()
        }
    }
    override fun beginTransaction() = db.beginTransaction()
    override fun setTransactionSuccessful() = db.setTransactionSuccessful()
    override fun endTransaction() = db.endTransaction()
    override fun inTransaction() = db.inTransaction()
    override fun close() = db.close()
}
