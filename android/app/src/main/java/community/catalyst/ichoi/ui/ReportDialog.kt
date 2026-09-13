package community.catalyst.ichoi.ui

import android.app.AlertDialog
import android.content.Context
import android.text.InputType
import android.view.ViewGroup
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.Spinner
import android.widget.ArrayAdapter
import community.catalyst.csilgen.generated.ContentReportReason

object ReportDialog {
    const val MAX_DETAILS = 2_000

    fun show(context: Context, title: String, onSubmit: (ContentReportReason, String?) -> Unit) {
        val layout = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL; setPadding(32, 8, 32, 0) }
        val reason = Spinner(context).apply {
            adapter = ArrayAdapter(context, android.R.layout.simple_spinner_dropdown_item, ContentReportReason.values().map {
                when (it) {
                    ContentReportReason.ObjectionableContent -> "Objectionable content"
                    ContentReportReason.Harassment -> "Harassment"
                    ContentReportReason.Spam -> "Spam"
                    ContentReportReason.Other -> "Other"
                }
            })
        }
        val details = EditText(context).apply {
            hint = "Details (optional)"
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            minLines = 3
        }
        layout.addView(reason, ViewGroup.LayoutParams(-1, -2)); layout.addView(details, ViewGroup.LayoutParams(-1, -2))
        AlertDialog.Builder(context).setTitle(title).setView(layout).setNegativeButton("Cancel", null)
            .setPositiveButton("Send report") { _, _ ->
                val text = details.text.toString()
                if (text.codePointCount(0, text.length) <= MAX_DETAILS) {
                    onSubmit(ContentReportReason.values()[reason.selectedItemPosition], text.takeIf { it.isNotBlank() })
                } else {
                    AlertDialog.Builder(context)
                        .setMessage("Details are limited to 2,000 Unicode scalar values")
                        .setPositiveButton("OK", null)
                        .show()
                }
            }.show()
    }
}
