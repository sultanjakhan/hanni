package com.sultanjakhan.hanni

import android.app.Activity
import android.os.Bundle
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/** Health Connect opens this explanation on both Android 13 and Android 14+. */
class HealthPermissionsActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        title = "Данные здоровья в Hanni"
        val padding = (24 * resources.displayMetrics.density).toInt()
        val content = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(padding, padding, padding, padding)
            addView(TextView(this@HealthPermissionsActivity).apply {
                textSize = 22f
                text = "Данные здоровья в Hanni"
            })
            addView(TextView(this@HealthPermissionsActivity).apply {
                textSize = 17f
                setPadding(0, padding, 0, 0)
                text = "Hanni запрашивает чтение доступных типов данных о здоровье, включая сон, шаги, пульс, тренировки, питание и измерения тела, " +
                    "из Health Connect. Данные сохраняются в личном архиве; поддерживаемые представления используют их в разделах здоровья, календаре " +
                    "и истории активности.\n\n" +
                    "Импортированные данные сохраняются в базе Hanni на устройстве. " +
                    "При настроенной синхронизации они также передаются по выбранному каналу " +
                    "синхронизации Hanni.\n\n" +
                    "Доступ к истории позволяет импортировать более ранние записи. Фоновое чтение, если оно поддерживается системой и разрешено, позволяет " +
                    "обновлять данные при закрытом интерфейсе Hanni.\n\n" +
                    "Вы можете разрешить отдельные типы данных и отозвать доступ в настройках " +
                    "Health Connect. Отзыв доступа прекращает дальнейшее чтение, но сам по себе " +
                    "не удаляет уже импортированные записи из Hanni."
            })
        }
        setContentView(ScrollView(this).apply { addView(content) })
    }
}
