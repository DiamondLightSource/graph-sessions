package system

import data.token
import rego.v1

# METADATA
# description: Allow subjects on session or containing proposal
# entrypoint: true
main := {"allow": allow}

default allow := false

# Allow if the SKIP_AUTHORIZATION environment variable is set and a preset token is supplied
allow if {
	opa.runtime().env.SKIP_AUTHORIZATION
	input.token == "ValidToken"
}

# Allow if on proposal which contains session
allow if {
	some proposal_number in data.diamond.data.subjects[token.claims.fedid].proposals
	proposal_number == input.proposal
}

# Allow if directly on session
allow if {
	some session_id in data.diamond.data.subjects[token.claims.fedid].sessions
	session := data.diamond.data.sessions[session_id]
	session.proposal_number == input.proposal
	session.visit_number == input.visit
}
