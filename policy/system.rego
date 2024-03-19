package system

import rego.v1

main := {"allow": allow}

default allow := false

allow if {
    input.token == "ValidToken"
}
