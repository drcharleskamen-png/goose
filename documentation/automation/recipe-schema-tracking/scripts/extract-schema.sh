#!/bin/bash
# Extract and resolve Recipe schema from ACP schema at a specific git version.
# Older versions fall back to the legacy OpenAPI spec.
# Usage: ./extract-schema.sh <version>
# Example: ./extract-schema.sh v1.15.0

set -e

VERSION=${1:-"main"}
GOOSE_REPO=${GOOSE_REPO:-"$HOME/Development/goose"}

if [ ! -d "$GOOSE_REPO" ]; then
    echo "Error: GOOSE_REPO directory not found: $GOOSE_REPO" >&2
    exit 1
fi

cd "$GOOSE_REPO"

# Verify version exists (for non-main versions)
if [ "$VERSION" != "main" ]; then
    if ! git rev-parse "$VERSION" >/dev/null 2>&1; then
        echo "Error: Version $VERSION not found in git history" >&2
        exit 1
    fi
fi

# Extract ACP schema from git, falling back to the legacy OpenAPI spec for old tags.
if [ "$VERSION" = "main" ]; then
    if [ -f crates/goose/acp-schema.json ]; then
        SCHEMA_JSON=$(cat crates/goose/acp-schema.json)
    elif [ -f ui/desktop/openapi.json ]; then
        SCHEMA_JSON=$(cat ui/desktop/openapi.json)
    else
        echo "Error: neither crates/goose/acp-schema.json nor ui/desktop/openapi.json found in working directory" >&2
        exit 1
    fi
else
    SCHEMA_JSON=$(git show "$VERSION:crates/goose/acp-schema.json" 2>/dev/null || git show "$VERSION:ui/desktop/openapi.json" 2>/dev/null || {
        echo "Error: Could not find crates/goose/acp-schema.json or ui/desktop/openapi.json at version $VERSION" >&2
        exit 1
    })
fi

# Use Node.js to extract and resolve Recipe schema
echo "$SCHEMA_JSON" | node -e "
const schemaDoc = JSON.parse(require('fs').readFileSync(0, 'utf-8'));

/**
 * Resolves \$ref references by expanding them with the actual schema definitions
 * Ported from ui/desktop/src/recipe/validation.ts
 */
function resolveRefs(schema, schemaDoc) {
  if (!schema || typeof schema !== 'object') {
    return schema;
  }

  // Handle \$ref
  if (typeof schema.\$ref === 'string') {
    const refPath = schema.\$ref.replace('#/', '').split('/');
    let resolved = schemaDoc;

    for (const segment of refPath) {
      if (resolved && typeof resolved === 'object' && segment in resolved) {
        resolved = resolved[segment];
      } else {
        console.warn(\`Could not resolve \$ref: \${schema.\$ref}\`);
        return schema; // Return original if can't resolve
      }
    }

    if (resolved && typeof resolved === 'object') {
      // Recursively resolve refs in the resolved schema
      return resolveRefs(resolved, schemaDoc);
    }

    return schema;
  }

  // Handle allOf (merge schemas)
  if (Array.isArray(schema.allOf)) {
    const merged = {};
    for (const subSchema of schema.allOf) {
      if (typeof subSchema === 'object' && subSchema !== null) {
        const resolved = resolveRefs(subSchema, schemaDoc);
        Object.assign(merged, resolved);
      }
    }
    // Keep other properties from the original schema
    const { allOf, ...rest } = schema;
    return { ...merged, ...rest };
  }

  // Handle oneOf/anyOf (keep as union)
  if (Array.isArray(schema.oneOf)) {
    return {
      ...schema,
      oneOf: schema.oneOf.map((subSchema) =>
        typeof subSchema === 'object' && subSchema !== null
          ? resolveRefs(subSchema, schemaDoc)
          : subSchema
      ),
    };
  }

  if (Array.isArray(schema.anyOf)) {
    return {
      ...schema,
      anyOf: schema.anyOf.map((subSchema) =>
        typeof subSchema === 'object' && subSchema !== null
          ? resolveRefs(subSchema, schemaDoc)
          : subSchema
      ),
    };
  }

  // Handle object properties
  if (schema.type === 'object' && schema.properties && typeof schema.properties === 'object') {
    const resolvedProperties = {};
    for (const [key, value] of Object.entries(schema.properties)) {
      if (typeof value === 'object' && value !== null) {
        resolvedProperties[key] = resolveRefs(value, schemaDoc);
      } else {
        resolvedProperties[key] = value;
      }
    }
    return {
      ...schema,
      properties: resolvedProperties,
    };
  }

  // Handle array items
  if (schema.type === 'array' && schema.items && typeof schema.items === 'object') {
    return {
      ...schema,
      items: resolveRefs(schema.items, schemaDoc),
    };
  }

  // Return schema as-is if no refs to resolve
  return schema;
}

// Extract Recipe schema from current ACP schema, or legacy OpenAPI schema.
const recipeSchema = schemaDoc.\$defs?.RecipeDto || schemaDoc.components?.schemas?.Recipe;

if (!recipeSchema) {
  console.error('Error: Recipe schema not found');
  process.exit(1);
}

// Resolve all \$refs in the schema
const resolvedSchema = resolveRefs(recipeSchema, schemaDoc);

// Convert OpenAPI schema to JSON Schema format
const jsonSchema = {
  '\$schema': 'http://json-schema.org/draft-07/schema#',
  ...resolvedSchema,
  title: resolvedSchema.title || 'Recipe',
  description: resolvedSchema.description || 'A Recipe represents a personalized, user-generated agent configuration that defines specific behaviors and capabilities within the Goose system.',
};

// Output the resolved schema
console.log(JSON.stringify(jsonSchema, null, 2));
"
