const RULES = {
    required: (val) => val != undefined && val !== "",
    minLength: (min) => (val) => val != undefined && val.length >= min,
    maxLength: (max) => (val) => val == null || val.length <= max,
    pattern: (re) => (val) => val == null || re.test(val),
    numeric: (val) => val == null || !isNaN(Number(val)),
    email: (val) => val == null || /^[^@]+@[^@]+\.[^@]+$/.test(val),
};

const ValidationError = class {
    constructor(field, message) {
        this.field = field;
        this.message = message;
    }

    toString() {
        return `${this.field}: ${this.message}`;
    }
};

const Validator = class {
    constructor(schema) {
        this.schema = schema;
        this.errors = [];
    }

    validate(data) {
        this.errors = [];

        for (const [field, rules] of Object.entries(this.schema)) {
            let value = data[field] == null ? null : data[field];

            for (const [ruleName, ruleArg] of Object.entries(rules)) {
                let ruleFn = typeof ruleArg === "function" ? ruleArg : RULES[ruleName];

                if (ruleFn == null) {
                    console.debug(`Unknown rule: ${ruleName}`);
                    continue;
                }

                if (Boolean(ruleFn(value)) === false) {
                    this.errors.push(new ValidationError(field, `failed rule: ${ruleName}`));
                }
            }
        }

        return this.errors.length === 0;
    }

    getErrors() {
        return { ...this._groupErrors() };
    }

    _groupErrors() {
        let grouped = {};
        for (var err of this.errors) {
            if (grouped[err.field] == null) {
                grouped[err.field] = [];
            }
            grouped[err.field].push(err.message);
        }
        return grouped;
    }

    hasErrors() {
        return Boolean(this.errors.length);
    }
};

function createValidator(schema) {
    return new Validator(schema);
}
