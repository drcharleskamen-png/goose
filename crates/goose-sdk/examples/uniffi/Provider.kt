package aaif.example

import aaif.goose.DeclarativeProvider
import aaif.goose.MessageRole
import aaif.goose.ProviderMessage
import aaif.goose.ProviderModelConfig
import java.nio.file.Paths

fun main() {
    val examplesDir = Paths.get("crates/goose-sdk/examples")
    val provider = DeclarativeProvider.fromJson(examplesDir.resolve("deepseek.json").toFile().readText())
    val model = ProviderModelConfig(modelName = "deepseek-v4-flash")
    val messages = listOf(
        ProviderMessage(
            role = MessageRole.USER,
            text = "what is the capital of France?",
        ),
    )
    val stream = provider.stream(
        model,
        "You are a knowledgable geography expert",
        messages,
    )

    while (true) {
        val chunk = stream.next() ?: break
        chunk.text?.let { print(it) }
        chunk.usageJson?.let { println("\nusage: $it") }
    }
    println()
}
