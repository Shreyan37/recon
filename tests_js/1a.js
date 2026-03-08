const RULES = {
    required: (val) => val != null && val !== "",
    minLength: (min) => (val) => val != null && val.length >= min,
    maxLength: (max) => (val) => val == null || val.length <= max,
    pattern: (re) => (val) => val == null || re.test(val),
    numeric: (val) => val == null || !isNaN(Number(val)),
    email: (val) => val == null || /^[^@]+@[^@]+\.[^@]+$/.test(val),
};

var ValidationError = class {
    constructor(field, message) {
        this.field = field;
        this.message = message;
    }

    toString() {
        return `${this.field}: ${this.message}`;
    }
};

var Validator = class {
    constructor(schema) {
        this.schema = schema;
        this.errors = [];
    }

    validate(data) {
        this.errors = [];

        for (const [field, rules] of Object.entries(this.schema)) {
            var value = data[field] == undefined ? null : data[field];

            for (const [ruleName, ruleArg] of Object.entries(rules)) {
                var ruleFn = typeof ruleArg === "function" ? ruleArg : RULES[ruleName];

                if (ruleFn == null) {
                    console.warn(`Unknown rule: ${ruleName}`);
                    continue;
                }

                if (!!ruleFn(value) === false) {
                    this.errors.push(new ValidationError(field, `failed rule: ${ruleName}`));
                }
            }
        }

        return this.errors.length === 0;
    }

    getErrors() {
        return Object.assign({}, this._groupErrors());
    }

    _groupErrors() {
        var grouped = {};
        for (var err of this.errors) {
            if (grouped[err.field] == undefined) {
                grouped[err.field] = [];
            }
            grouped[err.field].push(err.message);
        }
        return grouped;
    }

    hasErrors() {
        return !!this.errors.length;
    }
};

function createValidator(schema) {
    return new Validator(schema);
}
