import numpy as np
import pandas as pd

sep=','
header='infer'
train_df = pd.read_csv('./datasets/titanic/train.csv', header=header, sep=sep)
test_df = pd.read_csv('./datasets/titanic/test.csv', header=header, sep=sep)

output_file = open('/host/output.log', 'w', encoding="utf-8")

train_df.head()

null_value_stats = train_df.isnull().sum(axis=0)
null_value_stats[null_value_stats != 0]

train_df.fillna(-999, inplace=True)
test_df.fillna(-999, inplace=True)

X = train_df.drop('Survived', axis=1)
y = train_df.Survived

print(X.dtypes)

categorical_features_indices = np.where(X.dtypes != float)[0]

from sklearn.model_selection import train_test_split

X_train, X_validation, y_train, y_validation = train_test_split(X, y, train_size=0.75, random_state=42)

X_test = test_df

from catboost import CatBoostClassifier, Pool, metrics, cv
from sklearn.metrics import accuracy_score

model = CatBoostClassifier(
    custom_loss=[metrics.Accuracy()],
    random_seed=42
)

model.fit(
    X_train, y_train,
    cat_features=categorical_features_indices,
    eval_set=(X_validation, y_validation),
    logging_level='Verbose'
);

cv_params = model.get_params()
cv_params.update({
    'loss_function': metrics.Logloss()
})
cv_data = cv(
    Pool(X, y, cat_features=categorical_features_indices),
    cv_params
)

print('Best validation accuracy score: {:.2f}±{:.2f} on step {}'.format(
    np.max(cv_data['test-Accuracy-mean']),
    cv_data['test-Accuracy-std'][np.argmax(cv_data['test-Accuracy-mean'])],
    np.argmax(cv_data['test-Accuracy-mean'])
))
output_file.write('Best validation accuracy score: {:.2f}±{:.2f} on step {}\n'.format(
    np.max(cv_data['test-Accuracy-mean']),
    cv_data['test-Accuracy-std'][np.argmax(cv_data['test-Accuracy-mean'])],
    np.argmax(cv_data['test-Accuracy-mean'])
))

print('Precise validation accuracy score: {}'.format(np.max(cv_data['test-Accuracy-mean'])))
output_file.write('Precise validation accuracy score: {}\n'.format(np.max(cv_data['test-Accuracy-mean'])))

predictions = model.predict(X_test)
predictions_probs = model.predict_proba(X_test)
print(predictions[:10])
output_file.write('predictions: {} \n'.format(str(predictions[:10])))
print(predictions_probs[:10])
output_file.write('predictions_probs: {} \n'.format(str(predictions_probs[:10])))

output_file.close()
