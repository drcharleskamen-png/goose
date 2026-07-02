package aaif.example

import io.aaif.goose.Client

fun main() {
    val client = Client()
    val pong = client.ping("aaif.io")
    println(pong.message)
}
