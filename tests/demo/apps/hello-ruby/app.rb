#!/usr/bin/env ruby
# Plain TCPServer (stdlib 'socket') on purpose -- avoids needing bundler or
# any gems (webrick isn't stdlib on Ruby 3+), so this app stays detectable
# via a near-empty Gemfile without ever running `bundle install` for real.
require "socket"

port = (ENV["PORT"] || "8080").to_i
server = TCPServer.new("0.0.0.0", port)

loop do
  client = server.accept
  body = "hello from hello-ruby (pid #{Process.pid})\n"
  client.print "HTTP/1.1 200 OK\r\n"
  client.print "Content-Type: text/plain\r\n"
  client.print "Content-Length: #{body.bytesize}\r\n"
  client.print "Connection: close\r\n\r\n"
  client.print body
  client.close
end
