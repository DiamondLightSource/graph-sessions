package token

import rego.v1

fetch_jwks(url) := http.send({
	"url": jwks_url,
	"method": "GET",
	"force_cache": true,
	"force_cache_duration_seconds": 3600,
})

jwks_endpoint := opa.runtime().env.JWKS_ENDPOINT

unverified := io.jwt.decode(input.token)

jwt_header := unverified[0]

jwks_url := concat("?", [jwks_endpoint, urlquery.encode_object({"kid": jwt_header.kid})])

jwks := fetch_jwks(jwks_url).raw_body

valid := io.jwt.decode_verify(input.token, {
	"cert": jwks,
	"iss": "https://authn.diamond.ac.uk/realms/master",
	"aud": "account",
})

claims := valid[2]
