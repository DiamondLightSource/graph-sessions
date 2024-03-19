package token

fetch_jwks(url) := http.send({
	"url": jwks_url,
	"method": "GET",
	"force_cache": true,
	"force_cache_duration_seconds": 3600,
})

jwks_endpoint := opa.runtime().env.JWKS_ENDPOINT

token_unverified := io.jwt.decode(input.token)

token_jwt_header := token_unverified[0]

jwks_url := concat("?", [jwks_endpoint, urlquery.encode_object({"kid": token_jwt_header.kid})])

jwks := fetch_jwks(jwks_url).raw_body

token := token_unverified

if {
	io.jwt.verify_rs256(input.token, jwks)
}

claims := token[1]
